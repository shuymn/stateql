use std::collections::BTreeMap;

use stateql_core::{
    Column, DataType, DiffOp, EquivalencePolicy, Expr, Ident, Literal, Mode, Orchestrator,
    OrchestratorOptions, OrchestratorOutput, QualifiedName, SchemaObject, Table,
};

#[path = "support/fake_dialect.rs"]
mod fake_dialect;

use fake_dialect::{FakeDialect, test_connection_config};

const CURRENT_SQL: &str = "CURRENT_SQL";
const DESIRED_SQL: &str = "DESIRED_SQL";

fn ident(value: &str) -> Ident {
    Ident::unquoted(value)
}

fn qualified(schema: Option<&str>, name: &str) -> QualifiedName {
    QualifiedName {
        schema: schema.map(ident),
        name: ident(name),
    }
}

fn table(name: QualifiedName) -> Table {
    Table {
        name,
        columns: Vec::new(),
        primary_key: None,
        foreign_keys: Vec::new(),
        checks: Vec::new(),
        exclusions: Vec::new(),
        options: Default::default(),
        partition: None,
        renamed_from: None,
    }
}

fn table_with_quantity_default(name: QualifiedName, default: Expr) -> Table {
    let mut t = table(name);
    t.columns.push(Column {
        name: ident("quantity"),
        data_type: DataType::Integer,
        not_null: false,
        default: Some(default),
        identity: None,
        generated: None,
        comment: None,
        collation: None,
        renamed_from: None,
        extra: BTreeMap::new(),
    });
    t
}

fn dry_run_output_sql(output: OrchestratorOutput) -> String {
    match output {
        OrchestratorOutput::DryRunSql(sql) => sql,
        other => panic!("unexpected output: {other:?}"),
    }
}

#[test]
fn wires_enable_drop_and_renders_skipped_diagnostics() {
    let dialect = FakeDialect::default();
    dialect.set_export_schema_sql(CURRENT_SQL);
    dialect.set_parse_result(
        CURRENT_SQL,
        vec![SchemaObject::Table(table(qualified(
            Some("public"),
            "users",
        )))],
    );
    dialect.set_parse_result(DESIRED_SQL, vec![]);

    let orchestrator = Orchestrator::new(&dialect);
    let dry_run_without_drop = orchestrator
        .run(
            &test_connection_config(),
            DESIRED_SQL,
            OrchestratorOptions {
                mode: Mode::DryRun,
                enable_drop: false,
            },
        )
        .expect("dry-run should succeed when drop is disabled");

    assert!(dialect.generated_ops().is_empty());
    let dry_run_without_drop_sql = dry_run_output_sql(dry_run_without_drop);
    assert!(
        dry_run_without_drop_sql.contains("-- Skipped: DROP TABLE"),
        "dry-run should include skipped-drop diagnostics when enable_drop=false",
    );

    let dry_run_with_drop = orchestrator
        .run(
            &test_connection_config(),
            DESIRED_SQL,
            OrchestratorOptions {
                mode: Mode::DryRun,
                enable_drop: true,
            },
        )
        .expect("dry-run should succeed when drop is enabled");

    assert_eq!(
        dialect.generated_ops(),
        vec![DiffOp::DropTable(qualified(Some("public"), "users"))],
    );
    let dry_run_with_drop_sql = dry_run_output_sql(dry_run_with_drop);
    assert!(dry_run_with_drop_sql.contains("DROP TABLE users;"));
    assert!(
        !dry_run_with_drop_sql.contains("-- Skipped:"),
        "skip diagnostics are only rendered when drop suppression occurs",
    );
}

#[test]
fn wires_schema_search_path_into_diff_config() {
    let dialect_without_path = FakeDialect::default();
    dialect_without_path.set_schema_search_path(vec![]);
    dialect_without_path.set_export_schema_sql(CURRENT_SQL);
    dialect_without_path.set_parse_result(
        CURRENT_SQL,
        vec![SchemaObject::Table(table(qualified(
            Some("public"),
            "users",
        )))],
    );
    dialect_without_path.set_parse_result(
        DESIRED_SQL,
        vec![SchemaObject::Table(table(qualified(None, "users")))],
    );

    let output_without_path = Orchestrator::new(&dialect_without_path)
        .run(
            &test_connection_config(),
            DESIRED_SQL,
            OrchestratorOptions {
                mode: Mode::DryRun,
                enable_drop: true,
            },
        )
        .expect("dry-run should succeed");

    assert!(
        !dialect_without_path.generated_ops().is_empty(),
        "without search path wiring, unqualified desired table should not match qualified current table",
    );
    assert!(!dry_run_output_sql(output_without_path).is_empty());

    let dialect_with_path = FakeDialect::default();
    dialect_with_path.set_schema_search_path(vec!["public".to_string()]);
    dialect_with_path.set_export_schema_sql(CURRENT_SQL);
    dialect_with_path.set_parse_result(
        CURRENT_SQL,
        vec![SchemaObject::Table(table(qualified(
            Some("public"),
            "users",
        )))],
    );
    dialect_with_path.set_parse_result(
        DESIRED_SQL,
        vec![SchemaObject::Table(table(qualified(None, "users")))],
    );

    let output_with_path = Orchestrator::new(&dialect_with_path)
        .run(
            &test_connection_config(),
            DESIRED_SQL,
            OrchestratorOptions {
                mode: Mode::DryRun,
                enable_drop: true,
            },
        )
        .expect("dry-run should succeed");

    assert!(
        dialect_with_path.generated_ops().is_empty(),
        "search_path from adapter should be wired into DiffConfig",
    );
    assert!(dry_run_output_sql(output_with_path).is_empty());
}

#[test]
fn wires_equivalence_policy_into_diff_config() {
    #[derive(Debug)]
    struct CastLiteralExprPolicy;

    impl EquivalencePolicy for CastLiteralExprPolicy {
        fn is_equivalent_expr(&self, left: &Expr, right: &Expr) -> bool {
            matches!(
                (left, right),
                (Expr::Raw(raw), Expr::Literal(Literal::Integer(0))) if raw == "'0'::integer"
            ) || matches!(
                (left, right),
                (Expr::Literal(Literal::Integer(0)), Expr::Raw(raw)) if raw == "'0'::integer"
            )
        }
    }

    let strict = FakeDialect::default();
    strict.set_export_schema_sql(CURRENT_SQL);
    strict.set_parse_result(
        CURRENT_SQL,
        vec![SchemaObject::Table(table_with_quantity_default(
            qualified(Some("public"), "users"),
            Expr::Raw("'0'::integer".to_string()),
        ))],
    );
    strict.set_parse_result(
        DESIRED_SQL,
        vec![SchemaObject::Table(table_with_quantity_default(
            qualified(None, "users"),
            Expr::Literal(Literal::Integer(0)),
        ))],
    );

    Orchestrator::new(&strict)
        .run(
            &test_connection_config(),
            DESIRED_SQL,
            OrchestratorOptions {
                mode: Mode::DryRun,
                enable_drop: true,
            },
        )
        .expect("strict-policy dry-run should succeed");

    assert!(
        strict
            .generated_ops()
            .iter()
            .any(|op| matches!(op, DiffOp::AlterColumn { .. })),
        "without policy injection, the expression difference should produce AlterColumn",
    );

    let relaxed_policy: &'static dyn EquivalencePolicy =
        Box::leak(Box::new(CastLiteralExprPolicy)) as &'static dyn EquivalencePolicy;

    let relaxed = FakeDialect::with_policy(relaxed_policy);
    relaxed.set_export_schema_sql(CURRENT_SQL);
    relaxed.set_parse_result(
        CURRENT_SQL,
        vec![SchemaObject::Table(table_with_quantity_default(
            qualified(Some("public"), "users"),
            Expr::Raw("'0'::integer".to_string()),
        ))],
    );
    relaxed.set_parse_result(
        DESIRED_SQL,
        vec![SchemaObject::Table(table_with_quantity_default(
            qualified(None, "users"),
            Expr::Literal(Literal::Integer(0)),
        ))],
    );

    let output = Orchestrator::new(&relaxed)
        .run(
            &test_connection_config(),
            DESIRED_SQL,
            OrchestratorOptions {
                mode: Mode::DryRun,
                enable_drop: true,
            },
        )
        .expect("relaxed-policy dry-run should succeed");

    assert!(
        relaxed.generated_ops().is_empty(),
        "policy from dialect should be injected into DiffConfig",
    );
    assert!(dry_run_output_sql(output).is_empty());
}
