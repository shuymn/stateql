use stateql_core::{
    DiffConfig, DiffDiagnostics, DiffEngine, DiffOp, Ident, Privilege, PrivilegeObject,
    PrivilegeOp, QualifiedName, SchemaObject, SkippedOpKind, Table,
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

fn with_enable_drop(enable_drop: bool) -> DiffConfig {
    DiffConfig {
        enable_drop,
        ..DiffConfig::default()
    }
}

#[test]
fn enable_drop_false_suppresses_drop_table_and_keeps_skipped_diagnostics() {
    let engine = DiffEngine::new();
    let current = vec![SchemaObject::Table(table("users"))];

    let diff = engine
        .diff_with_diagnostics(&[], &current, &with_enable_drop(false))
        .expect("diff should succeed");

    assert!(
        diff.ops.is_empty(),
        "enable_drop=false must not emit destructive ops",
    );
    assert_eq!(diff.diagnostics.skipped_ops.len(), 1);
    assert_eq!(
        diff.diagnostics.skipped_ops[0].kind,
        SkippedOpKind::DropTable
    );
    assert_eq!(
        diff.diagnostics.skipped_ops[0].op,
        DiffOp::DropTable(qualified("users")),
    );
}

#[test]
fn suppression_diagnostics_include_revoke_payload_kind() {
    let revoke = DiffOp::Revoke(privilege_select_on_table("users"));
    let diagnostics = DiffDiagnostics::from_enable_drop(std::slice::from_ref(&revoke), &[]);

    assert_eq!(diagnostics.skipped_ops.len(), 1);
    assert_eq!(diagnostics.skipped_ops[0].kind, SkippedOpKind::Revoke);
    assert_eq!(diagnostics.skipped_ops[0].op, revoke);
}
