use stateql_core::{DiffConfig, DiffEngine, DiffOp, Ident, QualifiedName, SchemaObject, View};

fn ident(value: &str) -> Ident {
    Ident::unquoted(value)
}

fn qualified(name: &str) -> QualifiedName {
    QualifiedName {
        schema: Some(ident("public")),
        name: ident(name),
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

fn with_enable_drop(enable_drop: bool) -> DiffConfig {
    DiffConfig {
        enable_drop,
        ..DiffConfig::default()
    }
}

#[test]
fn rebuild_expands_to_unchanged_dependent_view() {
    let engine = DiffEngine::new();

    let current_base = view("base_v", "SELECT 1 AS c");
    let desired_base = view("base_v", "SELECT 2 AS c");

    let current_dep = view("dep_v", "SELECT c FROM base_v");
    let desired_dep = view("dep_v", "SELECT c FROM base_v");

    let desired = vec![
        SchemaObject::View(desired_base.clone()),
        SchemaObject::View(desired_dep.clone()),
    ];
    let current = vec![
        SchemaObject::View(current_base),
        SchemaObject::View(current_dep),
    ];

    let ops = engine
        .diff(&desired, &current, &with_enable_drop(true))
        .expect("diff should succeed");

    assert_eq!(
        ops,
        vec![
            DiffOp::DropView(qualified("dep_v")),
            DiffOp::DropView(qualified("base_v")),
            DiffOp::CreateView(desired_base),
            DiffOp::CreateView(desired_dep),
        ],
    );
}

#[test]
fn rebuild_drop_create_order_is_transitive() {
    let engine = DiffEngine::new();

    let current_base = view("base_v", "SELECT 1 AS c");
    let desired_base = view("base_v", "SELECT 2 AS c");

    let current_mid = view("mid_v", "SELECT c FROM base_v");
    let desired_mid = view("mid_v", "SELECT c FROM base_v");

    let current_leaf = view("leaf_v", "SELECT c FROM mid_v");
    let desired_leaf = view("leaf_v", "SELECT c FROM mid_v");

    let desired = vec![
        SchemaObject::View(desired_base.clone()),
        SchemaObject::View(desired_mid.clone()),
        SchemaObject::View(desired_leaf.clone()),
    ];
    let current = vec![
        SchemaObject::View(current_base),
        SchemaObject::View(current_mid),
        SchemaObject::View(current_leaf),
    ];

    let ops = engine
        .diff(&desired, &current, &with_enable_drop(true))
        .expect("diff should succeed");

    assert_eq!(
        ops,
        vec![
            DiffOp::DropView(qualified("leaf_v")),
            DiffOp::DropView(qualified("mid_v")),
            DiffOp::DropView(qualified("base_v")),
            DiffOp::CreateView(desired_base),
            DiffOp::CreateView(desired_mid),
            DiffOp::CreateView(desired_leaf),
        ],
    );
}
