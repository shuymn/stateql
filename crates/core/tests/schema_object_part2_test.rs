use std::collections::BTreeMap;

use stateql_core::{
    CheckConstraint, CheckOption, Comment, CommentTarget, DataType, Deferrable, Domain,
    EnumValuePosition, ExclusionConstraint, ExclusionElement, Expr, ForeignKey, ForeignKeyAction,
    Function, FunctionParam, FunctionParamMode, FunctionSecurity, Ident, Literal, NullsOrder,
    Policy, PolicyCommand, Privilege, PrivilegeObject, PrivilegeOp, QualifiedName, SchemaDef,
    SchemaObject, SortOrder, Trigger, TriggerEvent, TriggerForEach, TriggerTiming, TypeDef,
    TypeKind, Value, View, ViewSecurity, Volatility,
};

fn qualified(schema: Option<&str>, name: &str) -> QualifiedName {
    QualifiedName {
        schema: schema.map(Ident::unquoted),
        name: Ident::unquoted(name),
    }
}

#[test]
fn schema_object_remaining_top_level_variants_are_constructible() {
    let trigger = Trigger {
        name: qualified(Some("public"), "users_set_updated_at"),
        table: qualified(Some("public"), "users"),
        timing: TriggerTiming::Before,
        events: vec![TriggerEvent::Insert, TriggerEvent::Update],
        for_each: TriggerForEach::Row,
        when_clause: Some(Expr::Is {
            expr: Box::new(Expr::Ident(Ident::unquoted("active"))),
            test: stateql_core::IsTest::NotNull,
        }),
        body: "EXECUTE FUNCTION set_updated_at()".to_string(),
    };
    let function = Function {
        name: qualified(Some("public"), "set_updated_at"),
        params: vec![FunctionParam {
            name: Some(Ident::unquoted("arg")),
            data_type: DataType::Integer,
            mode: Some(FunctionParamMode::In),
            default: Some(Expr::Literal(Literal::Integer(0))),
        }],
        return_type: Some(DataType::Integer),
        language: "plpgsql".to_string(),
        body: "BEGIN RETURN arg; END".to_string(),
        volatility: Some(Volatility::Stable),
        security: Some(FunctionSecurity::Definer),
    };
    let type_def = TypeDef {
        name: qualified(Some("public"), "account_state"),
        kind: TypeKind::Enum {
            labels: vec!["active".to_string(), "disabled".to_string()],
        },
    };
    let domain = Domain {
        name: qualified(Some("public"), "email_domain"),
        data_type: DataType::Text,
        default: None,
        not_null: true,
        checks: vec![CheckConstraint {
            name: Some(Ident::unquoted("email_domain_chk")),
            expr: Expr::Raw("VALUE <> ''".to_string()),
            no_inherit: false,
        }],
    };
    let extension = stateql_core::Extension {
        name: Ident::unquoted("pg_trgm"),
        schema: Some(Ident::unquoted("public")),
        version: Some("1.6".to_string()),
    };
    let schema = SchemaDef {
        name: Ident::unquoted("audit"),
    };
    let comment = Comment {
        target: CommentTarget::Column {
            table: qualified(Some("public"), "users"),
            column: Ident::unquoted("email"),
        },
        text: Some("normalized email".to_string()),
    };
    let mut privilege = Privilege::empty(
        PrivilegeObject::Table(qualified(Some("public"), "users")),
        Ident::unquoted("app_user"),
    );
    privilege.operations = vec![PrivilegeOp::Select, PrivilegeOp::Update];
    privilege.with_grant_option = true;
    let policy = Policy {
        name: Ident::unquoted("users_isolation"),
        table: qualified(Some("public"), "users"),
        command: Some(PolicyCommand::Select),
        using_expr: Some(Expr::Raw("tenant_id = current_tenant_id()".to_string())),
        check_expr: Some(Expr::Raw("tenant_id = current_tenant_id()".to_string())),
        roles: vec![Ident::unquoted("app_user")],
        permissive: true,
    };

    let objects = vec![
        SchemaObject::Trigger(trigger),
        SchemaObject::Function(function),
        SchemaObject::Type(type_def),
        SchemaObject::Domain(domain),
        SchemaObject::Extension(extension),
        SchemaObject::Schema(schema),
        SchemaObject::Comment(comment),
        SchemaObject::Privilege(privilege),
        SchemaObject::Policy(policy),
    ];

    assert!(matches!(objects[0], SchemaObject::Trigger(_)));
    assert!(matches!(objects[1], SchemaObject::Function(_)));
    assert!(matches!(objects[2], SchemaObject::Type(_)));
    assert!(matches!(objects[3], SchemaObject::Domain(_)));
    assert!(matches!(objects[4], SchemaObject::Extension(_)));
    assert!(matches!(objects[5], SchemaObject::Schema(_)));
    assert!(matches!(objects[6], SchemaObject::Comment(_)));
    assert!(matches!(objects[7], SchemaObject::Privilege(_)));
    assert!(matches!(objects[8], SchemaObject::Policy(_)));
}

#[test]
fn schema_object_supporting_types_are_constructible() {
    let check = CheckConstraint {
        name: Some(Ident::unquoted("users_age_check")),
        expr: Expr::Raw("age >= 0".to_string()),
        no_inherit: true,
    };
    let exclusion = ExclusionConstraint {
        name: Some(Ident::unquoted("users_booking_excl")),
        index_method: "gist".to_string(),
        elements: vec![ExclusionElement {
            expr: Expr::Ident(Ident::unquoted("room_id")),
            operator: "=".to_string(),
            opclass: None,
            order: Some(SortOrder::Asc),
            nulls: Some(NullsOrder::Last),
        }],
        where_clause: Some(Expr::Raw("cancelled = false".to_string())),
        deferrable: Some(Deferrable::Deferrable {
            initially_deferred: false,
        }),
    };
    let foreign_key = ForeignKey {
        name: Some(Ident::unquoted("users_org_fk")),
        columns: vec![Ident::unquoted("org_id")],
        referenced_table: qualified(Some("public"), "organizations"),
        referenced_columns: vec![Ident::unquoted("id")],
        on_delete: Some(ForeignKeyAction::Cascade),
        on_update: Some(ForeignKeyAction::NoAction),
        deferrable: Some(Deferrable::NotDeferrable),
        extra: BTreeMap::from([(String::from("mssql.not_for_replication"), Value::Bool(true))]),
    };
    let view = View {
        name: qualified(Some("public"), "active_users"),
        columns: vec![Ident::unquoted("id")],
        query: "SELECT id FROM users WHERE active = true".to_string(),
        check_option: Some(CheckOption::Cascaded),
        security: Some(ViewSecurity::Invoker),
        renamed_from: None,
    };
    let enum_position_before = EnumValuePosition::Before("disabled".to_string());
    let enum_position_after = EnumValuePosition::After("active".to_string());

    let type_composite = TypeKind::Composite {
        fields: vec![(Ident::unquoted("city"), DataType::Text)],
    };
    let type_range = TypeKind::Range {
        subtype: DataType::Timestamp {
            with_timezone: false,
        },
    };

    assert!(check.name.is_some());
    assert!(matches!(
        exclusion.deferrable,
        Some(Deferrable::Deferrable { .. })
    ));
    assert!(matches!(
        foreign_key.on_delete,
        Some(ForeignKeyAction::Cascade)
    ));
    assert!(matches!(view.check_option, Some(CheckOption::Cascaded)));
    assert!(matches!(view.security, Some(ViewSecurity::Invoker)));
    assert!(matches!(enum_position_before, EnumValuePosition::Before(_)));
    assert!(matches!(enum_position_after, EnumValuePosition::After(_)));
    assert!(matches!(type_composite, TypeKind::Composite { .. }));
    assert!(matches!(type_range, TypeKind::Range { .. }));
}
