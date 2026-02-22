use std::collections::BTreeMap;

use stateql_core::{
    Column, DataType, Dialect, DiffOp, Ident, IndexOwner, QualifiedName, Statement, Table,
};
use stateql_dialect_mssql::MssqlDialect;

#[test]
fn create_and_rename_ops_emit_batch_boundaries_and_sp_rename() {
    let dialect = MssqlDialect;
    let ops = vec![
        DiffOp::CreateTable(users_table()),
        DiffOp::RenameTable {
            from: qualified(Some("dbo"), "users"),
            to: qualified(Some("dbo"), "user_accounts"),
        },
        DiffOp::RenameColumn {
            table: qualified(Some("dbo"), "user_accounts"),
            from: Ident::unquoted("username"),
            to: Ident::unquoted("user_name"),
        },
        DiffOp::RenameIndex {
            owner: IndexOwner::Table(qualified(Some("dbo"), "user_accounts")),
            from: Ident::unquoted("ix_username"),
            to: Ident::unquoted("ix_user_name"),
        },
    ];

    let statements = dialect
        .generate_ddl(&ops)
        .expect("create/rename ops should be generated");

    assert_eq!(
        statements,
        vec![
            sql(
                "CREATE TABLE [dbo].[users] ([id] INT NOT NULL, [username] NVARCHAR(255) NOT NULL);"
            ),
            Statement::BatchBoundary,
            sql("EXEC sp_rename 'dbo.users', 'user_accounts';"),
            Statement::BatchBoundary,
            sql("EXEC sp_rename 'dbo.user_accounts.username', 'user_name', 'COLUMN';"),
            Statement::BatchBoundary,
            sql("EXEC sp_rename 'dbo.user_accounts.ix_username', 'ix_user_name', 'INDEX';"),
        ]
    );
}

fn users_table() -> Table {
    let mut table = Table::named("users");
    table.name = qualified(Some("dbo"), "users");
    table.columns = vec![
        Column {
            name: Ident::unquoted("id"),
            data_type: DataType::Integer,
            not_null: true,
            default: None,
            identity: None,
            generated: None,
            comment: None,
            collation: None,
            renamed_from: None,
            extra: BTreeMap::new(),
        },
        Column {
            name: Ident::unquoted("username"),
            data_type: DataType::Varchar { length: Some(255) },
            not_null: true,
            default: None,
            identity: None,
            generated: None,
            comment: None,
            collation: None,
            renamed_from: None,
            extra: BTreeMap::new(),
        },
    ];
    table
}

fn sql(text: &str) -> Statement {
    Statement::Sql {
        sql: text.to_string(),
        transactional: true,
        context: None,
    }
}

fn qualified(schema: Option<&str>, name: &str) -> QualifiedName {
    QualifiedName {
        schema: schema.map(Ident::unquoted),
        name: Ident::unquoted(name),
    }
}
