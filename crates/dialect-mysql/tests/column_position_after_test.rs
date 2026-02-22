use std::collections::BTreeMap;

use stateql_core::{
    Column, ColumnPosition, DataType, Dialect, DiffOp, Ident, QualifiedName, Statement,
};
use stateql_dialect_mysql::MysqlDialect;

#[test]
fn add_column_after_position_is_preserved() {
    let dialect = MysqlDialect;
    let table = qualified_name("users");

    let ops = vec![
        DiffOp::AddColumn {
            table: table.clone(),
            column: Box::new(sample_column("nickname")),
            position: Some(ColumnPosition::After(ident("email"))),
        },
        DiffOp::AddColumn {
            table,
            column: Box::new(sample_column("initials")),
            position: Some(ColumnPosition::After(ident("nickname"))),
        },
    ];

    let statements = dialect
        .generate_ddl(&ops)
        .expect("mysql generator should preserve AFTER positioning");

    let sql = joined_sql(&statements);
    assert!(
        sql.contains("ADD COLUMN `nickname`"),
        "first add-column statement is missing: {sql}"
    );
    assert!(
        sql.contains("AFTER `email`"),
        "first AFTER position is missing: {sql}"
    );
    assert!(
        sql.contains("ADD COLUMN `initials`"),
        "second add-column statement is missing: {sql}"
    );
    assert!(
        sql.contains("AFTER `nickname`"),
        "second AFTER position is missing: {sql}"
    );

    let first_index = sql
        .find("`nickname`")
        .expect("nickname column token should be present");
    let second_index = sql
        .find("`initials`")
        .expect("initials column token should be present");
    assert!(
        first_index < second_index,
        "column-position order must remain stable across emitted SQL: {sql}"
    );
}

fn sample_column(name: &str) -> Column {
    Column {
        name: ident(name),
        data_type: DataType::Varchar { length: Some(255) },
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

fn joined_sql(statements: &[Statement]) -> String {
    statements
        .iter()
        .map(|statement| match statement {
            Statement::Sql { sql, .. } => sql.as_str(),
            Statement::BatchBoundary => "<BATCH>",
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn qualified_name(name: &str) -> QualifiedName {
    QualifiedName {
        schema: None,
        name: ident(name),
    }
}

fn ident(value: &str) -> Ident {
    Ident::unquoted(value)
}
