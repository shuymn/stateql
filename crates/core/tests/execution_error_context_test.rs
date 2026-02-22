use stateql_core::{
    Error, ExecutionError, Executor, Ident, QualifiedName, SqliteRebuildStep, Statement,
    StatementContext,
};

#[path = "support/fake_adapter.rs"]
mod fake_adapter;

use fake_adapter::FakeAdapter;

#[test]
fn execute_plan_propagates_failed_statement_context() {
    let mut adapter = FakeAdapter::default();
    let failing_sql = "INSERT INTO _new_users SELECT id, legacy FROM users;";
    adapter.set_fail_on_sql(failing_sql, "copy failed");

    let sqlite_rebuild_context = StatementContext::SqliteTableRebuild {
        table: QualifiedName {
            schema: Some(Ident::unquoted("main")),
            name: Ident::unquoted("users"),
        },
        step: SqliteRebuildStep::CopyData,
    };

    let statements = vec![
        Statement::Sql {
            sql: "CREATE TABLE _new_users (id INTEGER, legacy TEXT);".to_string(),
            transactional: true,
            context: None,
        },
        Statement::Sql {
            sql: failing_sql.to_string(),
            transactional: true,
            context: Some(sqlite_rebuild_context.clone()),
        },
    ];

    let mut executor = Executor::new(&mut adapter);
    let error = executor
        .execute_plan(&statements)
        .expect_err("statement execution should fail");

    let Error::Execute(ExecutionError::StatementFailed {
        statement_index,
        sql,
        executed_statements,
        statement_context,
        ..
    }) = error
    else {
        panic!("expected execution stage error");
    };

    assert_eq!(statement_index, 1);
    assert_eq!(sql, failing_sql);
    assert_eq!(executed_statements, 1);
    assert_eq!(statement_context.as_deref(), Some(&sqlite_rebuild_context));
}
