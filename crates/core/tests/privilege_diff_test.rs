use stateql_core::{
    DiffConfig, DiffEngine, DiffOp, Ident, Privilege, PrivilegeObject, PrivilegeOp, QualifiedName,
    SchemaObject,
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

fn privilege(operations: Vec<PrivilegeOp>, with_grant_option: bool) -> Privilege {
    Privilege {
        operations,
        on: PrivilegeObject::Table(qualified("users")),
        grantee: ident("app_user"),
        with_grant_option,
    }
}

fn with_enable_drop(enable_drop: bool) -> DiffConfig {
    DiffConfig {
        enable_drop,
        ..DiffConfig::default()
    }
}

#[test]
fn emits_only_grant_for_added_operations() {
    let engine = DiffEngine::new();
    let desired = vec![SchemaObject::Privilege(privilege(
        vec![PrivilegeOp::Select, PrivilegeOp::Insert],
        false,
    ))];
    let current = vec![SchemaObject::Privilege(privilege(
        vec![PrivilegeOp::Select],
        false,
    ))];

    let ops = engine
        .diff(&desired, &current, &with_enable_drop(true))
        .expect("diff should succeed");

    assert_eq!(
        ops,
        vec![DiffOp::Grant(privilege(vec![PrivilegeOp::Insert], false))]
    );
}

#[test]
fn emits_only_revoke_for_removed_operations() {
    let engine = DiffEngine::new();
    let desired = vec![SchemaObject::Privilege(privilege(
        vec![PrivilegeOp::Select],
        false,
    ))];
    let current = vec![SchemaObject::Privilege(privilege(
        vec![PrivilegeOp::Select, PrivilegeOp::Insert],
        false,
    ))];

    let ops = engine
        .diff(&desired, &current, &with_enable_drop(true))
        .expect("diff should succeed");

    assert_eq!(
        ops,
        vec![DiffOp::Revoke(privilege(vec![PrivilegeOp::Insert], false))]
    );
}

#[test]
fn emits_with_grant_option_deltas_for_shared_operations() {
    let engine = DiffEngine::new();
    let shared_ops = vec![PrivilegeOp::Select];

    let upgrade = engine
        .diff(
            &[SchemaObject::Privilege(privilege(shared_ops.clone(), true))],
            &[SchemaObject::Privilege(privilege(
                shared_ops.clone(),
                false,
            ))],
            &with_enable_drop(true),
        )
        .expect("diff should succeed");

    assert_eq!(
        upgrade,
        vec![DiffOp::Grant(privilege(shared_ops.clone(), true))]
    );

    let downgrade = engine
        .diff(
            &[SchemaObject::Privilege(privilege(
                shared_ops.clone(),
                false,
            ))],
            &[SchemaObject::Privilege(privilege(shared_ops.clone(), true))],
            &with_enable_drop(true),
        )
        .expect("diff should succeed");

    assert_eq!(downgrade, vec![DiffOp::Revoke(privilege(shared_ops, true))]);
}

#[test]
fn compares_expanded_all_operation_sets_as_set_difference() {
    let engine = DiffEngine::new();
    let desired = vec![SchemaObject::Privilege(privilege(
        vec![
            PrivilegeOp::Trigger,
            PrivilegeOp::Delete,
            PrivilegeOp::Insert,
            PrivilegeOp::Update,
            PrivilegeOp::Select,
            PrivilegeOp::References,
            PrivilegeOp::Truncate,
        ],
        false,
    ))];
    let current = vec![SchemaObject::Privilege(privilege(
        vec![
            PrivilegeOp::Select,
            PrivilegeOp::Insert,
            PrivilegeOp::Update,
            PrivilegeOp::Delete,
            PrivilegeOp::Truncate,
            PrivilegeOp::References,
            PrivilegeOp::Trigger,
        ],
        false,
    ))];

    let ops = engine
        .diff(&desired, &current, &with_enable_drop(true))
        .expect("diff should succeed");

    assert!(
        ops.is_empty(),
        "expanded-all operation sets should be equal"
    );
}
