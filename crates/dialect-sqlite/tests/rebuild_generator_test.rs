use std::collections::BTreeMap;

use stateql_core::{
    Column, ColumnChange, ConnectionConfig, DataType, Dialect, DiffOp, Error, ExecutionError,
    Executor, Ident, QualifiedName, SqliteRebuildStep, Statement, StatementContext,
};
use stateql_dialect_sqlite::SqliteDialect;

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
    let config = in_memory_connection();
    let mut adapter = dialect
        .connect(&config)
        .expect("in-memory sqlite connection should succeed");

    adapter
        .execute("CREATE TABLE users (age INTEGER);")
        .expect("table creation should succeed");
    adapter
        .execute("INSERT INTO users(age) VALUES (NULL);")
        .expect("seed row should succeed");

    let statements = dialect
        .generate_ddl(&[DiffOp::AlterColumn {
            table: qualified(None, "users"),
            column: ident("age"),
            changes: vec![ColumnChange::SetNotNull(true)],
        }])
        .expect("rebuild plan should generate");

    let mut executor = Executor::new(adapter.as_mut());
    let error = executor
        .execute_plan(&statements)
        .expect_err("copy step should fail for NULL -> NOT NULL migration");

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
    assert_eq!(table, &qualified(None, "users"));
    assert_eq!(*step, SqliteRebuildStep::CopyData);

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
