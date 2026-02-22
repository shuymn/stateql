use stateql_core::{
    ColumnChange, DataType, Dialect, DiffOp, Expr, Ident, Literal, QualifiedName, Statement,
};
use stateql_dialect_mysql::MysqlDialect;

#[test]
fn merges_same_column_alterations_into_single_change_column_statement() {
    let dialect = MysqlDialect;
    let table = qualified_name("users");

    let ops = vec![
        DiffOp::AlterColumn {
            table: table.clone(),
            column: ident("age"),
            changes: vec![
                ColumnChange::SetType(DataType::BigInt),
                ColumnChange::SetNotNull(true),
            ],
        },
        DiffOp::AlterColumn {
            table,
            column: ident("age"),
            changes: vec![
                ColumnChange::SetDefault(Some(Expr::Literal(Literal::Integer(0)))),
                ColumnChange::SetCollation(Some("utf8mb4_bin".to_string())),
            ],
        },
    ];

    let statements = dialect
        .generate_ddl(&ops)
        .expect("mysql generator should merge compatible ALTER COLUMN ops");

    let sql = joined_sql(&statements);
    let uppercase = sql.to_ascii_uppercase();
    assert_eq!(
        uppercase.matches("CHANGE COLUMN").count(),
        1,
        "expected one merged CHANGE COLUMN statement, got: {sql}"
    );
    assert!(
        uppercase.contains(
            "ALTER TABLE `USERS` CHANGE COLUMN `AGE` `AGE` BIGINT NOT NULL DEFAULT 0 COLLATE UTF8MB4_BIN"
        ),
        "merged CHANGE COLUMN definition is missing expected clauses: {sql}"
    );
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
