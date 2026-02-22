use std::collections::BTreeMap;

use stateql_core::{
    Column, ColumnChange, DataType, Dialect, DiffOp, Executor, Ident, QualifiedName,
    SqliteRebuildStep, Statement, StatementContext,
};
use stateql_dialect_sqlite::SqliteDialect;

#[path = "support/sqlite_atomicity_fixture.rs"]
mod sqlite_atomicity_fixture;

use sqlite_atomicity_fixture::{
    assert_copy_step_failure, assert_rollback_left_original_table, prepare_users_with_null_age,
    set_not_null_age_op,
};

#[test]
fn add_column_without_position_generates_simple_alter_statement() {
    let dialect = SqliteDialect;
    let add_column = DiffOp::AddColumn {
        table: qualified(None, "users"),
        column: Box::new(sample_column("nickname")),
        position: None,
    };

    let statements = dialect
        .generate_ddl(&[add_column])
        .expect("sqlite add-column should generate SQL");

    assert_eq!(statements.len(), 1);
    let Statement::Sql {
        sql,
        transactional,
        context,
    } = &statements[0]
    else {
        panic!("expected SQL statement");
    };
    assert!(sql.starts_with("ALTER TABLE"));
    assert!(sql.contains("ADD COLUMN"));
    assert!(*transactional);
    assert!(context.is_none());
}

#[test]
fn alter_column_rewrites_to_sqlite_rebuild_steps_with_context() {
    let dialect = SqliteDialect;
    let table = qualified(None, "users");
    let alter_column = DiffOp::AlterColumn {
        table: table.clone(),
        column: ident("age"),
        changes: vec![ColumnChange::SetNotNull(true)],
    };

    let statements = dialect
        .generate_ddl(&[alter_column])
        .expect("sqlite rebuild should generate SQL");

    assert!(
        statements.len() >= 4,
        "rebuild should emit a multi-step statement sequence"
    );

    let mut saw_copy_step = false;
    for statement in &statements {
        let Statement::Sql {
            transactional,
            context,
            ..
        } = statement
        else {
            panic!("sqlite generator should not emit BatchBoundary");
        };
        assert!(transactional, "rebuild statements must be transactional");
        let Some(StatementContext::SqliteTableRebuild {
            table: context_table,
            step,
        }) = context
        else {
            panic!("rebuild statements must include sqlite rebuild context");
        };
        assert_eq!(context_table, &table);
        saw_copy_step |= *step == SqliteRebuildStep::CopyData;
    }

    assert!(saw_copy_step, "rebuild must include a copy-data step");
}

#[test]
fn sqlite_rebuild_copy_failure_rolls_back_entire_transaction() {
    let dialect = SqliteDialect;
    let mut adapter = prepare_users_with_null_age(&dialect);

    let statements = dialect
        .generate_ddl(&[set_not_null_age_op()])
        .expect("rebuild plan should generate");

    let mut executor = Executor::new(adapter.as_mut());
    let error = executor
        .execute_plan(&statements)
        .expect_err("copy step should fail for NULL -> NOT NULL migration");

    assert_copy_step_failure(error);
    assert_rollback_left_original_table(adapter.as_ref());
}

fn ident(value: &str) -> Ident {
    Ident::unquoted(value)
}

fn qualified(schema: Option<&str>, name: &str) -> QualifiedName {
    QualifiedName {
        schema: schema.map(Ident::unquoted),
        name: Ident::unquoted(name),
    }
}

fn sample_column(name: &str) -> Column {
    Column {
        name: ident(name),
        data_type: DataType::Text,
        not_null: false,
        default: None,
        identity: None,
        generated: None,
        comment: None,
        collation: None,
        renamed_from: None,
        extra: BTreeMap::new(),
    }
}
