use std::collections::BTreeMap;

use stateql_core::{
    Column, ColumnPosition, DataType, Expr, GeneratedColumn, Ident, Identity, IndexColumn,
    IndexDef, IndexOwner, Literal, MaterializedView, Partition, PartitionBound, PartitionElement,
    PartitionStrategy, PrimaryKey, QualifiedName, SchemaObject, Sequence, SetQuantifier, Table,
    TableOptions, Value, View,
};

fn qualified(schema: Option<&str>, name: &str) -> QualifiedName {
    QualifiedName {
        schema: schema.map(Ident::unquoted),
        name: Ident::unquoted(name),
    }
}

#[test]
fn schema_object_top_level_variants_are_constructible() {
    let table = Table::named("users");
    let view = View::new(
        qualified(Some("public"), "active_users"),
        "SELECT * FROM users",
    );
    let materialized_view = MaterializedView {
        name: qualified(Some("public"), "user_summary"),
        columns: vec![Column {
            name: Ident::unquoted("id"),
            data_type: DataType::BigInt,
            not_null: true,
            default: None,
            identity: None,
            generated: None,
            comment: None,
            collation: None,
            renamed_from: None,
            extra: BTreeMap::new(),
        }],
        query: "SELECT id FROM users".to_string(),
        options: TableOptions::default(),
        renamed_from: None,
    };
    let index = IndexDef {
        name: Some(Ident::unquoted("idx_users_name")),
        owner: IndexOwner::Table(qualified(Some("public"), "users")),
        columns: vec![IndexColumn {
            expr: Expr::Ident(Ident::unquoted("name")),
        }],
        unique: false,
        method: Some("btree".to_string()),
        where_clause: Some(Expr::Comparison {
            left: Box::new(Expr::Ident(Ident::unquoted("active"))),
            op: stateql_core::ComparisonOp::Equal,
            right: Box::new(Expr::Literal(Literal::Boolean(true))),
            quantifier: Some(SetQuantifier::Any),
        }),
        concurrent: false,
        extra: BTreeMap::new(),
    };
    let sequence = Sequence {
        name: qualified(Some("public"), "users_id_seq"),
        data_type: Some(DataType::BigInt),
        increment: Some(1),
        min_value: Some(1),
        max_value: None,
        start: Some(1),
        cache: Some(1),
        cycle: false,
        owned_by: Some((qualified(Some("public"), "users"), Ident::unquoted("id"))),
    };

    let objects = vec![
        SchemaObject::Table(table),
        SchemaObject::View(view),
        SchemaObject::MaterializedView(materialized_view),
        SchemaObject::Index(index),
        SchemaObject::Sequence(sequence),
    ];

    assert!(matches!(objects[0], SchemaObject::Table(_)));
    assert!(matches!(objects[1], SchemaObject::View(_)));
    assert!(matches!(objects[2], SchemaObject::MaterializedView(_)));
    assert!(matches!(objects[3], SchemaObject::Index(_)));
    assert!(matches!(objects[4], SchemaObject::Sequence(_)));
}

#[test]
fn table_index_partition_and_column_position_types_are_constructible() {
    let mut table = Table::named("accounts");
    table.columns.push(Column {
        name: Ident::unquoted("id"),
        data_type: DataType::BigInt,
        not_null: true,
        default: None,
        identity: Some(Identity {
            always: true,
            start: Some(1),
            increment: Some(1),
            min_value: None,
            max_value: None,
            cache: Some(1),
            cycle: false,
        }),
        generated: Some(GeneratedColumn {
            expr: Expr::Raw("id + 1".to_string()),
            stored: true,
        }),
        comment: Some("primary identifier".to_string()),
        collation: None,
        renamed_from: None,
        extra: BTreeMap::from([(String::from("mysql.auto_increment"), Value::Bool(true))]),
    });
    table.primary_key = Some(PrimaryKey {
        name: Some(Ident::unquoted("accounts_pkey")),
        columns: vec![Ident::unquoted("id")],
    });
    table.options = TableOptions {
        extra: BTreeMap::from([(String::from("postgres.fillfactor"), Value::Integer(70))]),
    };
    table.partition = Some(Partition {
        strategy: PartitionStrategy::Range,
        columns: vec![Ident::unquoted("created_at")],
        partitions: vec![PartitionElement {
            name: Ident::unquoted("p2026"),
            bound: Some(PartitionBound::LessThan(vec![Expr::Literal(
                Literal::String("2027-01-01".to_string()),
            )])),
            extra: BTreeMap::new(),
        }],
    });

    let first = ColumnPosition::First;
    let after = ColumnPosition::After(Ident::unquoted("id"));

    assert_eq!(table.name.name.value, "accounts");
    assert!(matches!(
        table.partition,
        Some(Partition {
            strategy: PartitionStrategy::Range,
            ..
        })
    ));
    assert!(matches!(first, ColumnPosition::First));
    assert!(matches!(
        after,
        ColumnPosition::After(Ident {
            value,
            quoted: false
        }) if value == "id"
    ));
}
