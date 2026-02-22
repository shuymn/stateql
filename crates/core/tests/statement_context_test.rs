use stateql_core::{Ident, QualifiedName, SqliteRebuildStep, Statement, StatementContext};

#[test]
fn statement_sql_can_hold_sqlite_rebuild_context() {
    let statement = Statement::Sql {
        sql: "INSERT INTO _new_users SELECT id FROM users".to_string(),
        transactional: true,
        context: Some(StatementContext::SqliteTableRebuild {
            table: QualifiedName {
                schema: Some(Ident::unquoted("main")),
                name: Ident::unquoted("users"),
            },
            step: SqliteRebuildStep::CopyData,
        }),
    };

    match statement {
        Statement::Sql {
            transactional,
            context,
            ..
        } => {
            assert!(transactional);

            let Some(StatementContext::SqliteTableRebuild { table, step }) = context else {
                panic!("expected sqlite rebuild context");
            };

            assert_eq!(table.schema, Some(Ident::unquoted("main")));
            assert_eq!(table.name, Ident::unquoted("users"));
            assert_eq!(step, SqliteRebuildStep::CopyData);
        }
        Statement::BatchBoundary => panic!("expected sql statement"),
    }
}
