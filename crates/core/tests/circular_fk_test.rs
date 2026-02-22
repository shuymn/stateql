use std::collections::BTreeMap;

use stateql_core::{
    DiffConfig, DiffEngine, DiffOp, ForeignKey, Ident, QualifiedName, SchemaObject, Table,
    TableOptions,
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

fn foreign_key(name: &str, referenced_table: &str) -> ForeignKey {
    ForeignKey {
        name: Some(ident(name)),
        columns: vec![ident("id")],
        referenced_table: qualified(referenced_table),
        referenced_columns: vec![ident("id")],
        on_delete: None,
        on_update: None,
        deferrable: None,
        extra: BTreeMap::new(),
    }
}

fn table_with_foreign_keys(name: &str, foreign_keys: Vec<ForeignKey>) -> Table {
    Table {
        name: qualified(name),
        columns: Vec::new(),
        primary_key: None,
        foreign_keys,
        checks: Vec::new(),
        exclusions: Vec::new(),
        options: TableOptions::default(),
        partition: None,
        renamed_from: None,
    }
}

fn with_enable_drop(enable_drop: bool) -> DiffConfig {
    DiffConfig {
        enable_drop,
        ..DiffConfig::default()
    }
}

#[test]
fn create_cycle_falls_back_to_add_foreign_key_ops() {
    let engine = DiffEngine::new();

    let desired = vec![
        SchemaObject::Table(table_with_foreign_keys(
            "a",
            vec![foreign_key("a_b_fk", "b")],
        )),
        SchemaObject::Table(table_with_foreign_keys(
            "b",
            vec![foreign_key("b_a_fk", "a")],
        )),
    ];

    let ops = engine
        .diff(&desired, &[], &with_enable_drop(false))
        .expect("circular create dependencies should use fallback, not fail");

    assert_eq!(
        ops,
        vec![
            DiffOp::CreateTable(table_with_foreign_keys("a", Vec::new())),
            DiffOp::CreateTable(table_with_foreign_keys("b", Vec::new())),
            DiffOp::AddForeignKey {
                table: qualified("a"),
                fk: foreign_key("a_b_fk", "b"),
            },
            DiffOp::AddForeignKey {
                table: qualified("b"),
                fk: foreign_key("b_a_fk", "a"),
            },
        ]
    );
}

#[test]
fn self_referential_fk_is_not_treated_as_cycle() {
    let engine = DiffEngine::new();
    let self_fk = foreign_key("employees_manager_fk", "employees");
    let desired = vec![SchemaObject::Table(table_with_foreign_keys(
        "employees",
        vec![self_fk.clone()],
    ))];

    let ops = engine
        .diff(&desired, &[], &with_enable_drop(false))
        .expect("self-referential fk should not trigger cycle fallback");

    assert!(
        !ops.iter()
            .any(|op| matches!(op, DiffOp::AddForeignKey { .. })),
        "self-referential fk must remain embedded in CreateTable",
    );
    assert_eq!(ops.len(), 1);
    assert_eq!(
        ops[0],
        DiffOp::CreateTable(table_with_foreign_keys("employees", vec![self_fk])),
    );
}

#[test]
fn drop_cycle_emits_drop_foreign_keys_before_drop_tables_with_declaration_order() {
    let engine = DiffEngine::new();
    let current = vec![
        SchemaObject::Table(table_with_foreign_keys(
            "b",
            vec![foreign_key("b_a_fk", "a")],
        )),
        SchemaObject::Table(table_with_foreign_keys(
            "a",
            vec![foreign_key("a_b_fk", "b")],
        )),
    ];

    let ops = engine
        .diff(&[], &current, &with_enable_drop(true))
        .expect("circular drop dependencies should use fallback, not fail");

    assert_eq!(
        ops,
        vec![
            DiffOp::DropForeignKey {
                table: qualified("b"),
                name: ident("b_a_fk"),
            },
            DiffOp::DropForeignKey {
                table: qualified("a"),
                name: ident("a_b_fk"),
            },
            DiffOp::DropTable(qualified("b")),
            DiffOp::DropTable(qualified("a")),
        ]
    );
}
