use std::collections::BTreeMap;

use stateql_core::{
    Dialect, DiffOp, Expr, Ident, Literal, Partition, PartitionBound, PartitionElement,
    PartitionStrategy, QualifiedName, SchemaObject, Statement, View,
};
use stateql_dialect_mysql::MysqlDialect;

#[test]
fn add_partition_renders_partition_sql() {
    let dialect = MysqlDialect;

    let ops = vec![DiffOp::AddPartition {
        table: qualified_name("orders"),
        partition: Partition {
            strategy: PartitionStrategy::Range,
            columns: vec![ident("tenant_id")],
            partitions: vec![PartitionElement {
                name: ident("p0"),
                bound: Some(PartitionBound::LessThan(vec![Expr::Literal(
                    Literal::Integer(1000),
                )])),
                extra: BTreeMap::new(),
            }],
        },
    }];

    let statements = dialect
        .generate_ddl(&ops)
        .expect("mysql partition add should generate SQL");
    let sql = joined_sql(&statements);

    assert!(
        sql.contains("PARTITION BY RANGE (`tenant_id`)"),
        "expected partition strategy clause: {sql}"
    );
    assert!(
        sql.contains("PARTITION `p0` VALUES LESS THAN (1000)"),
        "expected partition element clause: {sql}"
    );
}

#[test]
fn drop_and_create_view_uses_create_or_replace_for_same_view_name() {
    let dialect = MysqlDialect;
    let view_name = qualified_name("active_users");
    let replacement = View {
        name: view_name.clone(),
        columns: vec![ident("id")],
        query: "SELECT id FROM users WHERE active = 1".to_string(),
        check_option: None,
        security: None,
        renamed_from: None,
    };

    let statements = dialect
        .generate_ddl(&[
            DiffOp::DropView(view_name),
            DiffOp::CreateView(replacement.clone()),
        ])
        .expect("drop+create view pair should be optimizable");

    assert_eq!(statements.len(), 1, "expected merged CREATE OR REPLACE");
    let Statement::Sql { sql, .. } = &statements[0] else {
        panic!("view replacement should emit SQL statement");
    };
    let upper = sql.to_ascii_uppercase();
    assert!(
        upper.contains("CREATE OR REPLACE VIEW"),
        "expected CREATE OR REPLACE VIEW optimization: {sql}"
    );

    let roundtrip = dialect
        .to_sql(&SchemaObject::View(replacement))
        .expect("to_sql should render view");
    assert!(
        roundtrip.contains("CREATE VIEW"),
        "baseline renderer should still emit view SQL: {roundtrip}"
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
