use std::collections::BTreeMap;

use stateql_core::{
    CheckConstraint, DataType, DiffConfig, DiffEngine, DiffOp, Expr, Ident, QualifiedName,
    SchemaObject, Table,
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

fn with_enable_drop(enable_drop: bool) -> DiffConfig {
    DiffConfig {
        enable_drop,
        ..DiffConfig::default()
    }
}

fn table_with_checks(name: &str, checks: Vec<CheckConstraint>) -> Table {
    Table {
        name: qualified(name),
        columns: vec![stateql_core::Column {
            name: ident("quantity"),
            data_type: DataType::Integer,
            not_null: false,
            default: None,
            identity: None,
            generated: None,
            comment: None,
            collation: None,
            renamed_from: None,
            extra: BTreeMap::new(),
        }],
        primary_key: None,
        foreign_keys: Vec::new(),
        checks,
        exclusions: Vec::new(),
        options: Default::default(),
        partition: None,
        renamed_from: None,
    }
}

fn named_check(name: &str, expr: &str) -> CheckConstraint {
    CheckConstraint {
        name: Some(ident(name)),
        expr: Expr::Raw(expr.to_string()),
        no_inherit: false,
    }
}

#[test]
fn enable_drop_false_keeps_drop_and_add_for_modified_named_check() {
    let engine = DiffEngine::new();

    let desired_check = named_check("users_quantity_check", "quantity > 10");
    let desired = vec![SchemaObject::Table(table_with_checks(
        "users",
        vec![desired_check.clone()],
    ))];
    let current = vec![SchemaObject::Table(table_with_checks(
        "users",
        vec![named_check("users_quantity_check", "quantity > 0")],
    ))];

    let ops = engine
        .diff(&desired, &current, &with_enable_drop(false))
        .expect("diff should succeed");

    assert_eq!(
        ops,
        vec![
            DiffOp::DropCheck {
                table: qualified("users"),
                name: ident("users_quantity_check"),
            },
            DiffOp::AddCheck {
                table: qualified("users"),
                check: desired_check,
            },
        ]
    );
}

#[test]
fn enable_drop_false_still_suppresses_unpaired_check_drop() {
    let engine = DiffEngine::new();
    let current = vec![SchemaObject::Table(table_with_checks(
        "users",
        vec![named_check("users_quantity_check", "quantity > 0")],
    ))];

    let ops = engine
        .diff(&[], &current, &with_enable_drop(false))
        .expect("diff should succeed");

    assert!(
        ops.is_empty(),
        "unpaired destructive check drop must stay suppressed",
    );
}
