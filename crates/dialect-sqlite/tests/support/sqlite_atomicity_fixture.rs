use std::collections::BTreeMap;

use stateql_core::{
    ColumnChange, ConnectionConfig, DatabaseAdapter, Dialect, DiffOp, Error, ExecutionError, Ident,
    QualifiedName, SqliteRebuildStep, StatementContext,
};
use stateql_dialect_sqlite::SqliteDialect;

pub fn prepare_users_with_null_age(dialect: &SqliteDialect) -> Box<dyn DatabaseAdapter> {
    let adapter = dialect
        .connect(&in_memory_connection())
        .expect("in-memory sqlite connection should succeed");
    adapter
        .execute("CREATE TABLE users (age INTEGER);")
        .expect("table creation should succeed");
    adapter
        .execute("INSERT INTO users(age) VALUES (NULL);")
        .expect("seed row should succeed");
    adapter
}

pub fn set_not_null_age_op() -> DiffOp {
    DiffOp::AlterColumn {
        table: users_table(),
        column: ident("age"),
        changes: vec![ColumnChange::SetNotNull(true)],
    }
}

pub fn assert_copy_step_failure(error: Error) {
    let Error::Execute(ExecutionError::StatementFailed {
        statement_context, ..
    }) = error
    else {
        panic!("expected execution-stage error");
    };
    let Some(StatementContext::SqliteTableRebuild { table, step }) = statement_context.as_deref()
    else {
        panic!("expected sqlite rebuild context");
    };
    assert_eq!(table, &users_table());
    assert_eq!(*step, SqliteRebuildStep::CopyData);
}

pub fn assert_rollback_left_original_table(adapter: &dyn DatabaseAdapter) {
    adapter
        .execute("INSERT INTO users(age) VALUES (NULL);")
        .expect("original nullable table should remain after rollback");

    let exported = adapter
        .export_schema()
        .expect("export_schema should succeed after rollback");
    assert!(
        exported.contains("CREATE TABLE users"),
        "original table definition should remain present: {exported}"
    );
    assert!(
        !exported.contains("__stateql_rebuild_"),
        "shadow table should be rolled back on failure: {exported}"
    );
}

fn in_memory_connection() -> ConnectionConfig {
    ConnectionConfig {
        host: None,
        port: None,
        user: None,
        password: None,
        database: ":memory:".to_string(),
        socket: None,
        extra: BTreeMap::new(),
    }
}

fn users_table() -> QualifiedName {
    QualifiedName {
        schema: None,
        name: ident("users"),
    }
}

fn ident(value: &str) -> Ident {
    Ident::unquoted(value)
}
