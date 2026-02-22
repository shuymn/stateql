use std::collections::BTreeMap;

use stateql_core::{
    AnnotationAttachment, AnnotationTarget, DiffConfig, DiffEngine, DiffError, DiffOp, Error,
    ExecutionError, Executor, Expr, Ident, IndexColumn, IndexDef, IndexOwner, Mode, Orchestrator,
    OrchestratorOptions, ParseError, QualifiedName, RenameAnnotation, SchemaObject, Statement,
    Table, attach_annotations,
};

#[path = "support/fake_adapter.rs"]
mod fake_adapter;
#[path = "support/fake_dialect.rs"]
mod fake_dialect;

use fake_adapter::{BEGIN_SQL, COMMIT_SQL, FakeAdapter, ROLLBACK_SQL};
use fake_dialect::{FakeDialect, test_connection_config};

const CURRENT_SQL: &str = "CURRENT_SQL";
const UNKNOWN_DDL_SQL: &str = "CREATE TABLE users (id bigint);\nCREATE FOOBAR baz;";
const TX_SQL_1: &str = "CREATE TABLE users (id INT PRIMARY KEY);";
const TX_SQL_2: &str = "ALTER TABLE users ADD COLUMN name TEXT;";
const NON_TRANSACTIONAL_SQL: &str = "CREATE INDEX CONCURRENTLY idx_users_name ON users(name);";
const TX_SQL_3: &str = "ALTER TABLE users ADD COLUMN email TEXT;";
const TX_SQL_4: &str = "ALTER TABLE users ADD COLUMN age INT;";

fn ident(value: &str) -> Ident {
    Ident::unquoted(value)
}

fn qualified(name: &str) -> QualifiedName {
    QualifiedName {
        schema: Some(ident("public")),
        name: ident(name),
    }
}

fn table(name: &str) -> SchemaObject {
    SchemaObject::Table(Table {
        name: qualified(name),
        columns: Vec::new(),
        primary_key: None,
        foreign_keys: Vec::new(),
        checks: Vec::new(),
        exclusions: Vec::new(),
        options: Default::default(),
        partition: None,
        renamed_from: None,
    })
}

fn enable_drop_config() -> DiffConfig {
    DiffConfig {
        enable_drop: true,
        ..DiffConfig::default()
    }
}

fn table_index(table_name: &str, index_name: &str) -> IndexDef {
    IndexDef {
        name: Some(ident(index_name)),
        owner: IndexOwner::Table(qualified(table_name)),
        columns: vec![IndexColumn {
            expr: Expr::Ident(ident("id")),
        }],
        unique: false,
        method: Some("btree".to_string()),
        where_clause: None,
        concurrent: false,
        extra: BTreeMap::new(),
    }
}

fn attach_then_diff(
    desired: &mut [SchemaObject],
    current: &[SchemaObject],
    annotations: &[RenameAnnotation],
    attachments: &[AnnotationAttachment],
) -> stateql_core::Result<Vec<DiffOp>> {
    attach_annotations(desired, annotations, attachments)?;
    DiffEngine::new().diff(desired, current, &enable_drop_config())
}

#[test]
fn s1_unknown_ddl_fails_fast_without_diff_or_execution() {
    let dialect = FakeDialect::default();
    dialect.set_export_schema_sql(CURRENT_SQL);
    dialect.set_parse_result(CURRENT_SQL, vec![table("users")]);

    let orchestrator = Orchestrator::new(&dialect);
    let error = orchestrator
        .run(
            &test_connection_config(),
            UNKNOWN_DDL_SQL,
            OrchestratorOptions {
                mode: Mode::DryRun,
                enable_drop: true,
            },
        )
        .expect_err("unknown DDL must fail fast");

    match error {
        Error::Parse(ParseError::StatementConversion { source_sql, .. }) => {
            assert!(
                source_sql.contains("CREATE FOOBAR"),
                "expected unsupported statement in parse error source SQL",
            );
        }
        other => panic!("expected parse error, got {other:?}"),
    }

    assert!(
        dialect.generated_ops().is_empty(),
        "parse failure must prevent diff/generate from producing drop ops",
    );
    assert!(
        dialect.executed_sql().is_empty(),
        "parse failure must prevent execution",
    );
}

#[test]
fn s2_orphan_annotation_fails_before_drop_create_planning() {
    let mut desired = vec![table("new_name")];
    let current = vec![table("old_name")];
    let annotations = vec![RenameAnnotation {
        line: 2,
        from: ident("old_name"),
        deprecated_alias: false,
    }];
    let attachments = vec![AnnotationAttachment {
        line: 1,
        target: AnnotationTarget::Table(qualified("new_name")),
    }];

    let result = attach_then_diff(&mut desired, &current, &annotations, &attachments);
    assert!(matches!(
        result,
        Err(Error::Diff(DiffError::ObjectComparison { operation, .. }))
            if operation == "rename annotation mismatch"
    ));

    let SchemaObject::Table(table) = &desired[0] else {
        panic!("expected desired table object");
    };
    assert_eq!(
        table.renamed_from, None,
        "orphan annotations must not partially mutate rename metadata",
    );
}

#[test]
fn s3_non_transactional_failure_keeps_first_transaction_committed() {
    let mut adapter = FakeAdapter::default();
    adapter.set_fail_on_sql(NON_TRANSACTIONAL_SQL, "non-transactional failure");

    let statements = vec![
        Statement::Sql {
            sql: TX_SQL_1.to_string(),
            transactional: true,
            context: None,
        },
        Statement::Sql {
            sql: TX_SQL_2.to_string(),
            transactional: true,
            context: None,
        },
        Statement::Sql {
            sql: NON_TRANSACTIONAL_SQL.to_string(),
            transactional: false,
            context: None,
        },
        Statement::Sql {
            sql: TX_SQL_3.to_string(),
            transactional: true,
            context: None,
        },
        Statement::Sql {
            sql: TX_SQL_4.to_string(),
            transactional: true,
            context: None,
        },
    ];

    let mut executor = Executor::new(&mut adapter);
    let error = executor
        .execute_plan(&statements)
        .expect_err("non-transactional statement failure should bubble up");

    match error {
        Error::Execute(ExecutionError::StatementFailed {
            statement_index,
            sql,
            executed_statements,
            ..
        }) => {
            assert_eq!(statement_index, 2);
            assert_eq!(sql, NON_TRANSACTIONAL_SQL);
            assert_eq!(executed_statements, 2);
        }
        other => panic!("expected execution error, got {other:?}"),
    }

    assert_eq!(adapter.begin_count(), 1);
    assert_eq!(adapter.commit_count(), 1);
    assert_eq!(adapter.rollback_count(), 0);
    assert_eq!(
        adapter.executed_sql(),
        vec![
            BEGIN_SQL.to_string(),
            TX_SQL_1.to_string(),
            TX_SQL_2.to_string(),
            COMMIT_SQL.to_string(),
        ],
    );
}

#[test]
fn s4_batch_boundary_does_not_commit_and_failure_rolls_back_all_sql() {
    let mut adapter = FakeAdapter::default();
    adapter.set_fail_on_sql(TX_SQL_3, "transactional failure after batch boundary");

    let statements = vec![
        Statement::Sql {
            sql: TX_SQL_1.to_string(),
            transactional: true,
            context: None,
        },
        Statement::Sql {
            sql: TX_SQL_2.to_string(),
            transactional: true,
            context: None,
        },
        Statement::BatchBoundary,
        Statement::Sql {
            sql: TX_SQL_3.to_string(),
            transactional: true,
            context: None,
        },
    ];

    let mut executor = Executor::new(&mut adapter);
    let error = executor
        .execute_plan(&statements)
        .expect_err("failure after batch boundary should rollback the single transaction");

    match error {
        Error::Execute(ExecutionError::StatementFailed {
            statement_index,
            sql,
            executed_statements,
            ..
        }) => {
            assert_eq!(statement_index, 3);
            assert_eq!(sql, TX_SQL_3);
            assert_eq!(executed_statements, 2);
        }
        other => panic!("expected execution error, got {other:?}"),
    }

    assert_eq!(adapter.begin_count(), 1);
    assert_eq!(adapter.commit_count(), 0);
    assert_eq!(adapter.rollback_count(), 1);
    assert_eq!(
        adapter.executed_sql(),
        vec![
            BEGIN_SQL.to_string(),
            TX_SQL_1.to_string(),
            TX_SQL_2.to_string(),
            ROLLBACK_SQL.to_string(),
        ],
    );
}

#[test]
fn s5_missing_index_owner_fails_without_index_diff_ops() {
    let desired = vec![SchemaObject::Index(table_index(
        "nonexistent_table",
        "idx_missing",
    ))];

    let error = DiffEngine::new()
        .diff(&desired, &[], &enable_drop_config())
        .expect_err("missing index owner must fail fast");

    match error {
        Error::Diff(DiffError::ObjectComparison { target, operation }) => {
            assert!(
                target.contains("nonexistent_table"),
                "error target should mention missing owner",
            );
            assert!(
                operation.contains("owner"),
                "error operation should describe owner validation failure",
            );
        }
        other => panic!("expected diff object comparison error, got {other:?}"),
    }
}
