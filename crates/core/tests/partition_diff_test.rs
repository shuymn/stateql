use std::collections::BTreeMap;

use stateql_core::{
    DiffConfig, DiffEngine, DiffOp, Expr, Ident, Literal, Partition, PartitionBound,
    PartitionElement, PartitionStrategy, QualifiedName, SchemaObject, Table,
};

fn ident(value: &str) -> Ident {
    Ident::unquoted(value)
}

fn qualified(name: &str) -> QualifiedName {
    QualifiedName {
        schema: Some(ident("public")),
        name: ident(name),
    }
}

fn base_table(name: &str) -> Table {
    Table {
        name: qualified(name),
        columns: Vec::new(),
        primary_key: None,
        foreign_keys: Vec::new(),
        checks: Vec::new(),
        exclusions: Vec::new(),
        options: Default::default(),
        partition: None,
        renamed_from: None,
    }
}

fn partition_element(name: &str, bound: PartitionBound) -> PartitionElement {
    PartitionElement {
        name: ident(name),
        bound: Some(bound),
        extra: BTreeMap::new(),
    }
}

fn range_partition(elements: Vec<PartitionElement>) -> Partition {
    Partition {
        strategy: PartitionStrategy::Range,
        columns: vec![ident("created_at")],
        partitions: elements,
    }
}

fn with_enable_drop(enable_drop: bool) -> DiffConfig {
    DiffConfig {
        enable_drop,
        ..DiffConfig::default()
    }
}

#[test]
fn adds_partition_when_desired_has_partition_and_current_does_not() {
    let engine = DiffEngine::new();
    let mut desired_table = base_table("orders");
    let desired_partition = range_partition(vec![partition_element(
        "orders_2026",
        PartitionBound::LessThan(vec![Expr::Literal(Literal::Integer(20270101))]),
    )]);
    desired_table.partition = Some(desired_partition.clone());

    let current_table = base_table("orders");

    let ops = engine
        .diff(
            &[SchemaObject::Table(desired_table)],
            &[SchemaObject::Table(current_table)],
            &with_enable_drop(true),
        )
        .expect("diff should succeed");

    assert_eq!(
        ops,
        vec![DiffOp::AddPartition {
            table: qualified("orders"),
            partition: desired_partition,
        }],
    );
}

#[test]
fn drops_partition_when_current_has_partition_and_desired_does_not() {
    let engine = DiffEngine::new();

    let desired_table = base_table("orders");

    let mut current_table = base_table("orders");
    current_table.partition = Some(range_partition(vec![partition_element(
        "orders_2026",
        PartitionBound::LessThan(vec![Expr::Literal(Literal::Integer(20270101))]),
    )]));

    let ops = engine
        .diff(
            &[SchemaObject::Table(desired_table)],
            &[SchemaObject::Table(current_table)],
            &with_enable_drop(true),
        )
        .expect("diff should succeed");

    assert_eq!(
        ops,
        vec![DiffOp::DropPartition {
            table: qualified("orders"),
            name: ident("orders_2026"),
        }],
    );
}

#[test]
fn partition_element_change_is_decomposed_into_drop_and_add() {
    let engine = DiffEngine::new();

    let mut desired_table = base_table("orders");
    desired_table.partition = Some(range_partition(vec![partition_element(
        "orders_2026_h2",
        PartitionBound::LessThan(vec![Expr::Literal(Literal::Integer(20270701))]),
    )]));

    let mut current_table = base_table("orders");
    current_table.partition = Some(range_partition(vec![partition_element(
        "orders_2026_h1",
        PartitionBound::LessThan(vec![Expr::Literal(Literal::Integer(20270701))]),
    )]));

    let ops = engine
        .diff(
            &[SchemaObject::Table(desired_table.clone())],
            &[SchemaObject::Table(current_table)],
            &with_enable_drop(true),
        )
        .expect("diff should succeed");

    assert_eq!(ops.len(), 2);
    assert!(ops.contains(&DiffOp::DropPartition {
        table: qualified("orders"),
        name: ident("orders_2026_h1"),
    }));
    assert!(
        ops.contains(&DiffOp::AddPartition {
            table: qualified("orders"),
            partition: desired_table
                .partition
                .expect("partition should be set for add expectation"),
        })
    );
}

#[test]
fn treats_maxvalue_bound_change_as_partition_change() {
    let engine = DiffEngine::new();

    let mut desired_table = base_table("orders");
    desired_table.partition = Some(range_partition(vec![partition_element(
        "orders_future",
        PartitionBound::MaxValue,
    )]));

    let mut current_table = base_table("orders");
    current_table.partition = Some(range_partition(vec![partition_element(
        "orders_future",
        PartitionBound::LessThan(vec![Expr::Literal(Literal::Integer(20290101))]),
    )]));

    let ops = engine
        .diff(
            &[SchemaObject::Table(desired_table.clone())],
            &[SchemaObject::Table(current_table)],
            &with_enable_drop(true),
        )
        .expect("diff should succeed");

    assert_eq!(ops.len(), 2);
    assert!(ops.contains(&DiffOp::DropPartition {
        table: qualified("orders"),
        name: ident("orders_future"),
    }));
    assert!(
        ops.contains(&DiffOp::AddPartition {
            table: qualified("orders"),
            partition: desired_table
                .partition
                .expect("partition should be set for add expectation"),
        })
    );
}

#[test]
fn treats_from_to_bound_change_as_partition_change() {
    let engine = DiffEngine::new();

    let mut desired_table = base_table("orders");
    desired_table.partition = Some(range_partition(vec![partition_element(
        "orders_2026",
        PartitionBound::FromTo {
            from: vec![Expr::Literal(Literal::Integer(20260101))],
            to: vec![Expr::Literal(Literal::Integer(20270101))],
        },
    )]));

    let mut current_table = base_table("orders");
    current_table.partition = Some(range_partition(vec![partition_element(
        "orders_2026",
        PartitionBound::FromTo {
            from: vec![Expr::Literal(Literal::Integer(20250101))],
            to: vec![Expr::Literal(Literal::Integer(20260101))],
        },
    )]));

    let ops = engine
        .diff(
            &[SchemaObject::Table(desired_table.clone())],
            &[SchemaObject::Table(current_table)],
            &with_enable_drop(true),
        )
        .expect("diff should succeed");

    assert_eq!(
        ops,
        vec![
            DiffOp::DropPartition {
                table: qualified("orders"),
                name: ident("orders_2026"),
            },
            DiffOp::AddPartition {
                table: qualified("orders"),
                partition: desired_table
                    .partition
                    .expect("partition should be set for add expectation"),
            },
        ],
    );
}
