use std::collections::BTreeMap;

use stateql_core::{
    Column, DataType, Dialect, DiffOp, Ident, Identity, IndexColumn, IndexDef, IndexOwner,
    QualifiedName, Statement, Table, Value,
};
use stateql_dialect_mssql::MssqlDialect;

#[test]
fn identity_clustered_and_not_for_replication_are_rendered() {
    let dialect = MssqlDialect;

    let mut table = Table::named("users");
    table.name = qualified(Some("dbo"), "users");
    table.columns.push(Column {
        name: Ident::unquoted("id"),
        data_type: DataType::BigInt,
        not_null: true,
        default: None,
        identity: Some(Identity {
            always: false,
            start: Some(7),
            increment: Some(9),
            min_value: None,
            max_value: None,
            cache: None,
            cycle: false,
        }),
        generated: None,
        comment: None,
        collation: None,
        renamed_from: None,
        extra: BTreeMap::from([(
            "mssql.identity_not_for_replication".to_string(),
            Value::Bool(true),
        )]),
    });

    let index = IndexDef {
        name: Some(Ident::unquoted("ix_users_id")),
        owner: IndexOwner::Table(qualified(Some("dbo"), "users")),
        columns: vec![IndexColumn {
            expr: stateql_core::Expr::Ident(Ident::unquoted("id")),
        }],
        unique: true,
        method: Some("CLUSTERED".to_string()),
        where_clause: None,
        concurrent: false,
        extra: BTreeMap::new(),
    };

    let statements = dialect
        .generate_ddl(&[DiffOp::CreateTable(table), DiffOp::AddIndex(index)])
        .expect("identity/clustered ops should be generated");

    assert_eq!(statements.len(), 3);
    assert!(matches!(statements[1], Statement::BatchBoundary));

    let first_sql = statement_sql(&statements[0]);
    assert!(
        first_sql.contains("IDENTITY(7,9) NOT FOR REPLICATION"),
        "expected IDENTITY with NOT FOR REPLICATION, got: {first_sql}"
    );

    let second_sql = statement_sql(&statements[2]);
    assert!(
        second_sql.starts_with("CREATE UNIQUE CLUSTERED INDEX"),
        "expected clustered index SQL, got: {second_sql}"
    );
}

fn statement_sql(statement: &Statement) -> &str {
    match statement {
        Statement::Sql { sql, .. } => sql,
        Statement::BatchBoundary => panic!("expected SQL statement"),
    }
}

fn qualified(schema: Option<&str>, name: &str) -> QualifiedName {
    QualifiedName {
        schema: schema.map(Ident::unquoted),
        name: Ident::unquoted(name),
    }
}
