use std::collections::BTreeMap;

use stateql_core::{
    CheckConstraint, Column, DataType, DatabaseAdapter, DiffConfig, DiffEngine, DiffOp, Expr,
    ForeignKey, Ident, Privilege, PrivilegeObject, PrivilegeOp, QualifiedName, SchemaObject,
    SkippedOpKind, Table, View,
};

#[path = "support/fake_adapter.rs"]
mod fake_adapter;

use fake_adapter::{BEGIN_SQL, FakeAdapter, ROLLBACK_SQL};

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

fn table(name: &str) -> Table {
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

fn privilege_select_on_table(table: &str) -> Privilege {
    Privilege {
        operations: vec![PrivilegeOp::Select],
        on: PrivilegeObject::Table(qualified(table)),
        grantee: ident("app_role"),
        with_grant_option: false,
    }
}

fn named_check(name: &str, expr: &str) -> CheckConstraint {
    CheckConstraint {
        name: Some(ident(name)),
        expr: Expr::Raw(expr.to_string()),
        no_inherit: false,
    }
}

fn table_with_checks(name: &str, checks: Vec<CheckConstraint>) -> Table {
    Table {
        name: qualified(name),
        columns: vec![Column {
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
        options: Default::default(),
        partition: None,
        renamed_from: None,
    }
}

fn view(name: &str, query: &str) -> View {
    View {
        name: qualified(name),
        columns: vec![ident("c")],
        query: query.to_string(),
        check_option: None,
        security: None,
        renamed_from: None,
    }
}

#[test]
fn s6_transaction_raii_rollback_on_drop() {
    let mut adapter = FakeAdapter::default();

    {
        let mut tx = adapter.begin().expect("begin transaction");
        tx.execute("CREATE TABLE users (id INT);")
            .expect("execute inside transaction");
    }

    assert_eq!(
        adapter.executed_sql(),
        vec![
            BEGIN_SQL.to_string(),
            "CREATE TABLE users (id INT);".to_string(),
            ROLLBACK_SQL.to_string(),
        ],
    );
    assert_eq!(adapter.begin_count(), 1);
    assert_eq!(adapter.commit_count(), 0);
    assert_eq!(adapter.rollback_count(), 1);
}

#[test]
fn s7_enable_drop_suppresses_destructive_ops_and_reports_diagnostics() {
    let engine = DiffEngine::new();
    let current = vec![
        SchemaObject::Table(table("users")),
        SchemaObject::Privilege(privilege_select_on_table("users")),
    ];

    let outcome = engine
        .diff_with_diagnostics(&[], &current, &with_enable_drop(false))
        .expect("diff should succeed");

    assert!(
        outcome.ops.is_empty(),
        "enable_drop=false must suppress destructive diff ops",
    );
    let mut kinds = outcome
        .diagnostics
        .skipped_ops
        .iter()
        .map(|diagnostic| diagnostic.kind)
        .collect::<Vec<_>>();
    kinds.sort_by_key(|kind| kind.tag());
    assert_eq!(kinds, vec![SkippedOpKind::DropTable, SkippedOpKind::Revoke]);
}

#[test]
fn s8_enable_drop_false_still_allows_constraint_modification_pair() {
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
fn s9_circular_fk_create_uses_add_foreign_key_fallback() {
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
        .expect("circular create dependencies should use fallback");

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
        ],
    );
}

#[test]
fn s11_view_rebuild_expands_to_unchanged_dependents() {
    let engine = DiffEngine::new();
    let desired = vec![
        SchemaObject::View(view("base_v", "SELECT 2 AS c")),
        SchemaObject::View(view("dep_v", "SELECT c FROM base_v")),
    ];
    let current = vec![
        SchemaObject::View(view("base_v", "SELECT 1 AS c")),
        SchemaObject::View(view("dep_v", "SELECT c FROM base_v")),
    ];

    let ops = engine
        .diff(&desired, &current, &with_enable_drop(true))
        .expect("diff should succeed");

    assert_eq!(
        ops,
        vec![
            DiffOp::DropView(qualified("dep_v")),
            DiffOp::DropView(qualified("base_v")),
            DiffOp::CreateView(view("base_v", "SELECT 2 AS c")),
            DiffOp::CreateView(view("dep_v", "SELECT c FROM base_v")),
        ],
    );
}
