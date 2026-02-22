use std::collections::BTreeMap;

use stateql_core::{
    CheckConstraint, Comment, CommentTarget, DataType, DiffConfig, DiffEngine, DiffError, DiffOp,
    Domain, Error, Expr, Extension, Function, FunctionParam, FunctionParamMode, Ident, Identity,
    Literal, MaterializedView, Policy, PolicyCommand, Privilege, PrivilegeObject, QualifiedName,
    SchemaDef, SchemaObject, Sequence, SequenceChange, Table, Trigger, TriggerEvent,
    TriggerForEach, TriggerTiming, TypeChange, TypeDef, TypeKind, View,
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

fn unqualified(name: &str) -> QualifiedName {
    QualifiedName {
        schema: None,
        name: ident(name),
    }
}

fn with_enable_drop(enable_drop: bool) -> DiffConfig {
    DiffConfig {
        enable_drop,
        ..DiffConfig::default()
    }
}

fn plain_column(name: &str) -> stateql_core::Column {
    stateql_core::Column {
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

fn identity_column(name: &str) -> stateql_core::Column {
    stateql_core::Column {
        name: ident(name),
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
        generated: None,
        comment: None,
        collation: None,
        renamed_from: None,
        extra: BTreeMap::new(),
    }
}

fn table(name: &str) -> Table {
    Table {
        name: qualified(name),
        columns: vec![plain_column("id")],
        primary_key: None,
        foreign_keys: Vec::new(),
        checks: Vec::new(),
        exclusions: Vec::new(),
        options: Default::default(),
        partition: None,
        renamed_from: None,
    }
}

fn table_with_identity(name: &str, column_name: &str) -> Table {
    Table {
        name: qualified(name),
        columns: vec![identity_column(column_name)],
        primary_key: None,
        foreign_keys: Vec::new(),
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
        columns: vec![ident("id")],
        query: query.to_string(),
        check_option: None,
        security: None,
        renamed_from: None,
    }
}

fn materialized_view(name: &str, query: &str) -> MaterializedView {
    MaterializedView {
        name: qualified(name),
        columns: vec![plain_column("id")],
        query: query.to_string(),
        options: Default::default(),
        renamed_from: None,
    }
}

fn sequence(name: &str, increment: i64, cycle: bool) -> Sequence {
    Sequence {
        name: qualified(name),
        data_type: Some(DataType::BigInt),
        increment: Some(increment),
        min_value: Some(1),
        max_value: None,
        start: Some(1),
        cache: Some(1),
        cycle,
        owned_by: None,
    }
}

fn trigger(name: &str, table_name: &str, body: &str) -> Trigger {
    Trigger {
        name: qualified(name),
        table: qualified(table_name),
        timing: TriggerTiming::Before,
        events: vec![TriggerEvent::Insert],
        for_each: TriggerForEach::Row,
        when_clause: None,
        body: body.to_string(),
    }
}

fn function(name: &str, body: &str) -> Function {
    Function {
        name: qualified(name),
        params: vec![FunctionParam {
            name: Some(ident("input_id")),
            data_type: DataType::BigInt,
            mode: Some(FunctionParamMode::In),
            default: None,
        }],
        return_type: Some(DataType::BigInt),
        language: "plpgsql".to_string(),
        body: body.to_string(),
        volatility: None,
        security: None,
    }
}

fn enum_type(name: &str, labels: &[&str]) -> TypeDef {
    TypeDef {
        name: qualified(name),
        kind: TypeKind::Enum {
            labels: labels.iter().map(|label| (*label).to_string()).collect(),
        },
    }
}

fn domain(name: &str, default: Option<Expr>, not_null: bool, check_name: &str) -> Domain {
    Domain {
        name: qualified(name),
        data_type: DataType::Text,
        default,
        not_null,
        checks: vec![CheckConstraint {
            name: Some(ident(check_name)),
            expr: Expr::Raw("VALUE <> ''".to_string()),
            no_inherit: false,
        }],
    }
}

fn extension(name: &str, version: Option<&str>) -> Extension {
    Extension {
        name: ident(name),
        schema: Some(ident("public")),
        version: version.map(str::to_string),
    }
}

fn schema(name: &str) -> SchemaDef {
    SchemaDef { name: ident(name) }
}

fn comment(target_table: &str, text: Option<&str>) -> Comment {
    Comment {
        target: CommentTarget::Table(qualified(target_table)),
        text: text.map(str::to_string),
    }
}

fn policy(name: &str, table_name: &str, using_expr: &str) -> Policy {
    Policy {
        name: ident(name),
        table: qualified(table_name),
        command: Some(PolicyCommand::Select),
        using_expr: Some(Expr::Raw(using_expr.to_string())),
        check_expr: None,
        roles: vec![ident("app_user")],
        permissive: true,
    }
}

#[test]
fn diffs_view_and_materialized_view_create_drop_and_definition_change() {
    let engine = DiffEngine::new();

    let desired_view = view("users_view", "SELECT id FROM users WHERE active = true");
    let current_view = view("users_view", "SELECT id FROM users");

    let desired_mv = materialized_view("users_mv", "SELECT id FROM users WHERE active = true");
    let current_mv = materialized_view("users_mv", "SELECT id FROM users");

    let desired = vec![
        SchemaObject::View(desired_view.clone()),
        SchemaObject::View(view("create_view", "SELECT 1")),
        SchemaObject::MaterializedView(desired_mv.clone()),
        SchemaObject::MaterializedView(materialized_view("create_mv", "SELECT 1")),
    ];
    let current = vec![
        SchemaObject::View(current_view.clone()),
        SchemaObject::View(view("drop_view", "SELECT 1")),
        SchemaObject::MaterializedView(current_mv.clone()),
        SchemaObject::MaterializedView(materialized_view("drop_mv", "SELECT 1")),
    ];

    let ops = engine
        .diff(&desired, &current, &with_enable_drop(true))
        .expect("diff should succeed");

    assert!(ops.contains(&DiffOp::DropView(qualified("users_view"))));
    assert!(ops.contains(&DiffOp::CreateView(desired_view)));
    assert!(ops.contains(&DiffOp::CreateView(view("create_view", "SELECT 1"))));
    assert!(ops.contains(&DiffOp::DropView(qualified("drop_view"))));
    assert!(ops.contains(&DiffOp::DropMaterializedView(qualified("users_mv"))));
    assert!(ops.contains(&DiffOp::CreateMaterializedView(desired_mv)));
    assert!(
        ops.contains(&DiffOp::CreateMaterializedView(materialized_view(
            "create_mv",
            "SELECT 1",
        )))
    );
    assert!(ops.contains(&DiffOp::DropMaterializedView(qualified("drop_mv"))));
}

#[test]
fn diffs_sequence_trigger_function_type_and_domain_variants() {
    let engine = DiffEngine::new();

    let desired = vec![
        SchemaObject::Sequence(sequence("orders_id_seq", 10, true)),
        SchemaObject::Sequence(sequence("sequence_create", 1, false)),
        SchemaObject::Trigger(trigger(
            "users_trigger",
            "users",
            "EXECUTE FUNCTION users_new()",
        )),
        SchemaObject::Trigger(trigger("trigger_create", "users", "EXECUTE FUNCTION a()")),
        SchemaObject::Function(function("set_updated_at", "BEGIN RETURN 2; END")),
        SchemaObject::Function(function("function_create", "BEGIN RETURN 1; END")),
        SchemaObject::Type(enum_type("status", &["draft", "active"])),
        SchemaObject::Type(enum_type("type_create", &["one"])),
        SchemaObject::Domain(domain(
            "email_domain",
            Some(Expr::Literal(Literal::String("n/a".to_string()))),
            true,
            "domain_check_new",
        )),
        SchemaObject::Domain(domain(
            "domain_create",
            Some(Expr::Literal(Literal::String(String::new()))),
            true,
            "domain_create_check",
        )),
    ];
    let current = vec![
        SchemaObject::Sequence(sequence("orders_id_seq", 1, false)),
        SchemaObject::Sequence(sequence("sequence_drop", 1, false)),
        SchemaObject::Trigger(trigger(
            "users_trigger",
            "users",
            "EXECUTE FUNCTION users_old()",
        )),
        SchemaObject::Trigger(trigger("trigger_drop", "users", "EXECUTE FUNCTION a()")),
        SchemaObject::Function(function("set_updated_at", "BEGIN RETURN 1; END")),
        SchemaObject::Function(function("function_drop", "BEGIN RETURN 1; END")),
        SchemaObject::Type(enum_type("status", &["draft"])),
        SchemaObject::Type(enum_type("type_drop", &["one"])),
        SchemaObject::Domain(domain("email_domain", None, false, "domain_check_old")),
        SchemaObject::Domain(domain(
            "domain_drop",
            Some(Expr::Literal(Literal::String(String::new()))),
            true,
            "domain_drop_check",
        )),
    ];

    let ops = engine
        .diff(&desired, &current, &with_enable_drop(true))
        .expect("diff should succeed");

    assert!(ops.contains(&DiffOp::AlterSequence {
        name: qualified("orders_id_seq"),
        changes: vec![
            SequenceChange::SetIncrement(10),
            SequenceChange::SetCycle(true),
        ],
    }));
    assert!(ops.contains(&DiffOp::CreateSequence(sequence(
        "sequence_create",
        1,
        false,
    ))));
    assert!(ops.contains(&DiffOp::DropSequence(qualified("sequence_drop"))));

    assert!(ops.contains(&DiffOp::DropTrigger {
        name: qualified("users_trigger"),
        table: Some(qualified("users")),
    }));
    assert!(ops.contains(&DiffOp::CreateTrigger(trigger(
        "users_trigger",
        "users",
        "EXECUTE FUNCTION users_new()",
    ))));
    assert!(ops.contains(&DiffOp::CreateTrigger(trigger(
        "trigger_create",
        "users",
        "EXECUTE FUNCTION a()",
    ))));
    assert!(ops.contains(&DiffOp::DropTrigger {
        name: qualified("trigger_drop"),
        table: Some(qualified("users")),
    }));

    assert!(ops.contains(&DiffOp::DropFunction(qualified("set_updated_at"))));
    assert!(ops.contains(&DiffOp::CreateFunction(function(
        "set_updated_at",
        "BEGIN RETURN 2; END",
    ))));
    assert!(ops.contains(&DiffOp::CreateFunction(function(
        "function_create",
        "BEGIN RETURN 1; END",
    ))));
    assert!(ops.contains(&DiffOp::DropFunction(qualified("function_drop"))));

    assert!(ops.contains(&DiffOp::AlterType {
        name: qualified("status"),
        change: TypeChange::AddValue {
            value: "active".to_string(),
            position: None,
        },
    }));
    assert!(ops.contains(&DiffOp::CreateType(enum_type("type_create", &["one"]))));
    assert!(ops.contains(&DiffOp::DropType(qualified("type_drop"))));

    assert!(ops.contains(&DiffOp::AlterDomain {
        name: qualified("email_domain"),
        change: stateql_core::DomainChange::SetDefault(Some(Expr::Literal(Literal::String(
            "n/a".to_string(),
        )))),
    }));
    assert!(ops.contains(&DiffOp::AlterDomain {
        name: qualified("email_domain"),
        change: stateql_core::DomainChange::SetNotNull(true),
    }));
    assert!(ops.contains(&DiffOp::CreateDomain(domain(
        "domain_create",
        Some(Expr::Literal(Literal::String(String::new()))),
        true,
        "domain_create_check",
    ))));
    assert!(ops.contains(&DiffOp::DropDomain(qualified("domain_drop"))));
}

#[test]
fn diffs_extension_schema_comment_and_policy_variants() {
    let engine = DiffEngine::new();

    let desired = vec![
        SchemaObject::Extension(extension("pg_trgm", Some("1.6"))),
        SchemaObject::Extension(extension("hstore", None)),
        SchemaObject::Schema(schema("app")),
        SchemaObject::Comment(comment("users", Some("normalized"))),
        SchemaObject::Policy(policy(
            "users_rls",
            "users",
            "tenant_id = current_tenant_id()",
        )),
        SchemaObject::Policy(policy("policy_create", "users", "true")),
        SchemaObject::Table(table("users")),
    ];
    let current = vec![
        SchemaObject::Extension(extension("pg_trgm", Some("1.5"))),
        SchemaObject::Extension(extension("postgis", Some("3.4"))),
        SchemaObject::Schema(schema("legacy")),
        SchemaObject::Comment(comment("users", Some("old"))),
        SchemaObject::Comment(comment("drop_comment", Some("drop me"))),
        SchemaObject::Policy(policy("users_rls", "users", "tenant_id = 1")),
        SchemaObject::Policy(policy("policy_drop", "users", "true")),
        SchemaObject::Table(table("users")),
    ];

    let ops = engine
        .diff(&desired, &current, &with_enable_drop(true))
        .expect("diff should succeed");

    assert!(ops.contains(&DiffOp::DropExtension(qualified("pg_trgm"))));
    assert!(ops.contains(&DiffOp::CreateExtension(extension("pg_trgm", Some("1.6")))));
    assert!(ops.contains(&DiffOp::CreateExtension(extension("hstore", None))));
    assert!(ops.contains(&DiffOp::DropExtension(qualified("postgis"))));

    assert!(ops.contains(&DiffOp::CreateSchema(schema("app"))));
    assert!(ops.contains(&DiffOp::DropSchema(unqualified("legacy"))));

    assert!(ops.contains(&DiffOp::SetComment(comment("users", Some("normalized")))));
    assert!(ops.contains(&DiffOp::DropComment {
        target: CommentTarget::Table(qualified("drop_comment")),
    }));

    assert!(ops.contains(&DiffOp::DropPolicy {
        name: ident("users_rls"),
        table: qualified("users"),
    }));
    assert!(ops.contains(&DiffOp::CreatePolicy(policy(
        "users_rls",
        "users",
        "tenant_id = current_tenant_id()",
    ))));
    assert!(ops.contains(&DiffOp::CreatePolicy(policy(
        "policy_create",
        "users",
        "true",
    ))));
    assert!(ops.contains(&DiffOp::DropPolicy {
        name: ident("policy_drop"),
        table: qualified("users"),
    }));
}

#[test]
fn fails_fast_when_remaining_variant_is_not_supported_yet() {
    let engine = DiffEngine::new();
    let privilege = Privilege::empty(
        PrivilegeObject::Table(qualified("users")),
        ident("app_user"),
    );

    let error = engine
        .diff(
            &[
                SchemaObject::Table(table("users")),
                SchemaObject::Privilege(privilege),
            ],
            &[],
            &with_enable_drop(true),
        )
        .expect_err("unsupported variant should fail fast");

    match error {
        Error::Diff(DiffError::ObjectComparison { target, operation }) => {
            assert!(target.contains("privilege"));
            assert!(operation.contains("support"));
        }
        other => panic!("unexpected error: {other}"),
    }
}

#[test]
fn fails_when_desired_has_explicit_sequence_and_identity_overlap() {
    let engine = DiffEngine::new();

    let desired = vec![
        SchemaObject::Table(table_with_identity("users", "id")),
        SchemaObject::Sequence(sequence("users_id_seq", 1, false)),
    ];

    let error = engine
        .diff(&desired, &[], &with_enable_drop(true))
        .expect_err("sequence duplicate invariant violation must fail");

    match error {
        Error::Diff(DiffError::ObjectComparison { target, operation }) => {
            assert!(target.contains("users_id_seq"));
            assert!(operation.contains("desired"));
            assert!(operation.contains("identity"));
        }
        other => panic!("unexpected error: {other}"),
    }
}

#[test]
fn fails_when_current_has_explicit_sequence_and_identity_overlap() {
    let engine = DiffEngine::new();

    let current = vec![
        SchemaObject::Table(table_with_identity("users", "id")),
        SchemaObject::Sequence(sequence("users_id_seq", 1, false)),
    ];

    let error = engine
        .diff(&[], &current, &with_enable_drop(true))
        .expect_err("sequence duplicate invariant violation must fail");

    match error {
        Error::Diff(DiffError::ObjectComparison { target, operation }) => {
            assert!(target.contains("users_id_seq"));
            assert!(operation.contains("current"));
            assert!(operation.contains("identity"));
        }
        other => panic!("unexpected error: {other}"),
    }
}
