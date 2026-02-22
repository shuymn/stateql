use std::collections::BTreeMap;

use stateql_core::{
    Column, DataType, DiffConfig, DiffEngine, DiffOp, Ident, QualifiedName, SchemaObject, Table,
};

fn ident(value: &str) -> Ident {
    Ident::unquoted(value)
}

fn qualified(schema: &str, name: &str) -> QualifiedName {
    QualifiedName {
        schema: Some(ident(schema)),
        name: ident(name),
    }
}

fn unqualified(name: &str) -> QualifiedName {
    QualifiedName {
        schema: None,
        name: ident(name),
    }
}

fn column(name: &str) -> Column {
    Column {
        name: ident(name),
        data_type: DataType::BigInt,
        not_null: true,
        default: None,
        identity: None,
        generated: None,
        comment: None,
        collation: None,
        renamed_from: None,
        extra: BTreeMap::new(),
    }
}

fn table(name: QualifiedName, columns: Vec<Column>) -> Table {
    Table {
        name,
        columns,
        primary_key: None,
        foreign_keys: Vec::new(),
        checks: Vec::new(),
        exclusions: Vec::new(),
        options: Default::default(),
        partition: None,
        renamed_from: None,
    }
}

fn with_config(enable_drop: bool, search_path: &[&str]) -> DiffConfig {
    DiffConfig {
        enable_drop,
        schema_search_path: search_path
            .iter()
            .map(|schema| (*schema).to_string())
            .collect(),
        ..DiffConfig::default()
    }
}

#[test]
fn matches_unqualified_desired_with_qualified_current_when_schema_is_in_search_path() {
    let engine = DiffEngine::new();
    let desired = vec![SchemaObject::Table(table(
        unqualified("users"),
        vec![column("id")],
    ))];
    let current = vec![SchemaObject::Table(table(
        qualified("app", "users"),
        vec![column("id")],
    ))];

    let ops = engine
        .diff(&desired, &current, &with_config(true, &["app", "public"]))
        .expect("diff should succeed");

    assert!(ops.is_empty(), "expected no diff ops, got: {ops:?}");
}

#[test]
fn matches_qualified_desired_with_unqualified_current_when_schema_is_in_search_path() {
    let engine = DiffEngine::new();
    let desired = vec![SchemaObject::Table(table(
        qualified("app", "users"),
        vec![column("id")],
    ))];
    let current = vec![SchemaObject::Table(table(
        unqualified("users"),
        vec![column("id")],
    ))];

    let ops = engine
        .diff(&desired, &current, &with_config(true, &["app", "public"]))
        .expect("diff should succeed");

    assert!(ops.is_empty(), "expected no diff ops, got: {ops:?}");
}

#[test]
fn does_not_match_when_schema_is_not_in_search_path() {
    let engine = DiffEngine::new();
    let desired = vec![SchemaObject::Table(table(
        unqualified("users"),
        vec![column("id")],
    ))];
    let current = vec![SchemaObject::Table(table(
        qualified("app", "users"),
        vec![column("id")],
    ))];

    let ops = engine
        .diff(&desired, &current, &with_config(true, &["public"]))
        .expect("diff should succeed");

    assert_eq!(
        ops,
        vec![
            DiffOp::DropTable(qualified("app", "users")),
            DiffOp::CreateTable(table(unqualified("users"), vec![column("id")])),
        ],
    );
}

#[test]
fn prefers_first_matching_schema_in_search_path_order() {
    let engine = DiffEngine::new();
    let desired = vec![SchemaObject::Table(table(
        unqualified("users"),
        vec![column("id")],
    ))];
    let current = vec![
        SchemaObject::Table(table(
            qualified("public", "users"),
            vec![column("id"), column("public_only")],
        )),
        SchemaObject::Table(table(
            qualified("app", "users"),
            vec![column("id"), column("app_only")],
        )),
    ];

    let ops = engine
        .diff(&desired, &current, &with_config(true, &["app", "public"]))
        .expect("diff should succeed");

    assert!(
        ops.contains(&DiffOp::DropColumn {
            table: unqualified("users"),
            column: ident("app_only"),
        }),
        "expected app.users to be chosen as match, got: {ops:?}",
    );
    assert!(
        ops.contains(&DiffOp::DropTable(qualified("public", "users"))),
        "expected public.users to remain unmatched and be dropped, got: {ops:?}",
    );
}
