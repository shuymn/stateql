use std::collections::BTreeMap;

use stateql_core::{
    CheckConstraint, Column, ColumnChange, ColumnPosition, Comment, CommentTarget, DataType,
    Deferrable, DiffOp, Domain, DomainChange, EnumValuePosition, ExclusionConstraint,
    ExclusionElement, Expr, Extension, ForeignKey, ForeignKeyAction, Function, FunctionParam,
    FunctionParamMode, FunctionSecurity, GeneratedColumn, Ident, Identity, IndexColumn, IndexDef,
    IndexOwner, Literal, MaterializedView, NullsOrder, Partition, PartitionBound, PartitionElement,
    PartitionStrategy, Policy, PolicyCommand, PrimaryKey, Privilege, PrivilegeObject, PrivilegeOp,
    QualifiedName, SchemaDef, Sequence, SequenceChange, SetQuantifier, SortOrder, Table,
    TableOptions, Trigger, TriggerEvent, TriggerForEach, TriggerTiming, TypeChange, TypeDef,
    TypeKind, Value, View, Volatility,
};

pub const EXPECTED_DIFFOP_VARIANT_COUNT: usize = 48;
pub const EXPECTED_COLUMN_CHANGE_VARIANT_COUNT: usize = 6;
pub const EXPECTED_SEQUENCE_CHANGE_VARIANT_COUNT: usize = 7;
pub const EXPECTED_TYPE_CHANGE_VARIANT_COUNT: usize = 2;
pub const EXPECTED_DOMAIN_CHANGE_VARIANT_COUNT: usize = 4;

fn ident(value: &str) -> Ident {
    Ident::unquoted(value)
}

fn qualified(schema: Option<&str>, name: &str) -> QualifiedName {
    QualifiedName {
        schema: schema.map(Ident::unquoted),
        name: Ident::unquoted(name),
    }
}

fn literal_integer(value: i64) -> Expr {
    Expr::Literal(Literal::Integer(value))
}

fn sample_column(name: &str) -> Column {
    Column {
        name: ident(name),
        data_type: DataType::Varchar { length: Some(255) },
        not_null: false,
        default: Some(Expr::Literal(Literal::String("unknown".to_string()))),
        identity: None,
        generated: None,
        comment: Some("test column".to_string()),
        collation: Some("en_US".to_string()),
        renamed_from: None,
        extra: BTreeMap::from([(String::from("mysql.auto_increment"), Value::Bool(true))]),
    }
}

fn sample_table(name: &str) -> Table {
    Table {
        name: qualified(Some("public"), name),
        columns: vec![
            Column {
                name: ident("id"),
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
            },
            sample_column("email"),
        ],
        primary_key: Some(PrimaryKey {
            name: Some(ident("users_pkey")),
            columns: vec![ident("id")],
        }),
        foreign_keys: vec![ForeignKey {
            name: Some(ident("users_org_fk")),
            columns: vec![ident("org_id")],
            referenced_table: qualified(Some("public"), "organizations"),
            referenced_columns: vec![ident("id")],
            on_delete: Some(ForeignKeyAction::Cascade),
            on_update: Some(ForeignKeyAction::NoAction),
            deferrable: Some(Deferrable::Deferrable {
                initially_deferred: false,
            }),
            extra: BTreeMap::new(),
        }],
        checks: vec![CheckConstraint {
            name: Some(ident("users_email_check")),
            expr: Expr::Raw("email <> ''".to_string()),
            no_inherit: false,
        }],
        exclusions: vec![ExclusionConstraint {
            name: Some(ident("users_room_excl")),
            index_method: "gist".to_string(),
            elements: vec![ExclusionElement {
                expr: Expr::Ident(ident("room_id")),
                operator: "=".to_string(),
                opclass: None,
                order: Some(SortOrder::Asc),
                nulls: Some(NullsOrder::Last),
            }],
            where_clause: Some(Expr::Raw("cancelled = false".to_string())),
            deferrable: Some(Deferrable::NotDeferrable),
        }],
        options: TableOptions {
            extra: BTreeMap::from([(String::from("fillfactor"), Value::Integer(90))]),
        },
        partition: Some(Partition {
            strategy: PartitionStrategy::Range,
            columns: vec![ident("created_at")],
            partitions: vec![PartitionElement {
                name: ident("users_2026"),
                bound: Some(PartitionBound::FromTo {
                    from: vec![literal_integer(20260101)],
                    to: vec![literal_integer(20270101)],
                }),
                extra: BTreeMap::new(),
            }],
        }),
        renamed_from: None,
    }
}

fn sample_index() -> IndexDef {
    IndexDef {
        name: Some(ident("users_email_idx")),
        owner: IndexOwner::Table(qualified(Some("public"), "users")),
        columns: vec![IndexColumn {
            expr: Expr::Ident(ident("email")),
        }],
        unique: true,
        method: Some("btree".to_string()),
        where_clause: Some(Expr::Comparison {
            left: Box::new(Expr::Ident(ident("deleted_at"))),
            op: stateql_core::ComparisonOp::Equal,
            right: Box::new(Expr::Null),
            quantifier: Some(SetQuantifier::All),
        }),
        concurrent: false,
        extra: BTreeMap::new(),
    }
}

fn sample_view() -> View {
    View {
        name: qualified(Some("public"), "active_users"),
        columns: vec![ident("id"), ident("email")],
        query: "SELECT id, email FROM users WHERE active = true".to_string(),
        check_option: None,
        security: None,
        renamed_from: None,
    }
}

fn sample_materialized_view() -> MaterializedView {
    MaterializedView {
        name: qualified(Some("public"), "active_users_mv"),
        columns: vec![sample_column("id"), sample_column("email")],
        query: "SELECT id, email FROM users WHERE active = true".to_string(),
        options: TableOptions::default(),
        renamed_from: None,
    }
}

fn sample_sequence() -> Sequence {
    Sequence {
        name: qualified(Some("public"), "users_id_seq"),
        data_type: Some(DataType::BigInt),
        increment: Some(1),
        min_value: Some(1),
        max_value: None,
        start: Some(1),
        cache: Some(1),
        cycle: false,
        owned_by: Some((qualified(Some("public"), "users"), ident("id"))),
    }
}

fn sample_trigger() -> Trigger {
    Trigger {
        name: qualified(Some("public"), "users_set_updated_at"),
        table: qualified(Some("public"), "users"),
        timing: TriggerTiming::Before,
        events: vec![TriggerEvent::Insert, TriggerEvent::Update],
        for_each: TriggerForEach::Row,
        when_clause: Some(Expr::Is {
            expr: Box::new(Expr::Ident(ident("updated_at"))),
            test: stateql_core::IsTest::Null,
        }),
        body: "EXECUTE FUNCTION set_updated_at()".to_string(),
    }
}

fn sample_function() -> Function {
    Function {
        name: qualified(Some("public"), "set_updated_at"),
        params: vec![FunctionParam {
            name: Some(ident("input_id")),
            data_type: DataType::BigInt,
            mode: Some(FunctionParamMode::In),
            default: None,
        }],
        return_type: Some(DataType::Timestamp {
            with_timezone: false,
        }),
        language: "plpgsql".to_string(),
        body: "BEGIN RETURN now(); END".to_string(),
        volatility: Some(Volatility::Stable),
        security: Some(FunctionSecurity::Definer),
    }
}

fn sample_type() -> TypeDef {
    TypeDef {
        name: qualified(Some("public"), "status"),
        kind: TypeKind::Enum {
            labels: vec!["draft".to_string(), "active".to_string()],
        },
    }
}

fn sample_domain() -> Domain {
    Domain {
        name: qualified(Some("public"), "email_domain"),
        data_type: DataType::Text,
        default: Some(Expr::Literal(Literal::String(String::new()))),
        not_null: true,
        checks: vec![CheckConstraint {
            name: Some(ident("email_domain_check")),
            expr: Expr::Raw("VALUE <> ''".to_string()),
            no_inherit: false,
        }],
    }
}

fn sample_extension() -> Extension {
    Extension {
        name: ident("pg_trgm"),
        schema: Some(ident("public")),
        version: Some("1.6".to_string()),
    }
}

fn sample_schema() -> SchemaDef {
    SchemaDef {
        name: ident("reporting"),
    }
}

fn sample_comment() -> Comment {
    Comment {
        target: CommentTarget::Column {
            table: qualified(Some("public"), "users"),
            column: ident("email"),
        },
        text: Some("normalized email".to_string()),
    }
}

fn sample_privilege() -> Privilege {
    Privilege {
        operations: vec![PrivilegeOp::Select, PrivilegeOp::Update],
        on: PrivilegeObject::Table(qualified(Some("public"), "users")),
        grantee: ident("app_user"),
        with_grant_option: false,
    }
}

fn sample_policy() -> Policy {
    Policy {
        name: ident("users_isolation"),
        table: qualified(Some("public"), "users"),
        command: Some(PolicyCommand::Select),
        using_expr: Some(Expr::Raw("tenant_id = current_tenant_id()".to_string())),
        check_expr: Some(Expr::Raw("tenant_id = current_tenant_id()".to_string())),
        roles: vec![ident("app_user")],
        permissive: true,
    }
}

pub fn all_column_change_variants() -> Vec<ColumnChange> {
    vec![
        ColumnChange::SetType(DataType::Text),
        ColumnChange::SetNotNull(true),
        ColumnChange::SetDefault(Some(Expr::Literal(Literal::String(
            "fallback@example.com".to_string(),
        )))),
        ColumnChange::SetIdentity(Some(Identity {
            always: false,
            start: Some(1000),
            increment: Some(1),
            min_value: None,
            max_value: None,
            cache: Some(10),
            cycle: false,
        })),
        ColumnChange::SetGenerated(Some(GeneratedColumn {
            expr: Expr::Raw("lower(email)".to_string()),
            stored: true,
        })),
        ColumnChange::SetCollation(Some("und-x-icu".to_string())),
    ]
}

pub fn all_sequence_change_variants() -> Vec<SequenceChange> {
    vec![
        SequenceChange::SetType(DataType::BigInt),
        SequenceChange::SetIncrement(5),
        SequenceChange::SetMinValue(Some(1)),
        SequenceChange::SetMaxValue(Some(9_999)),
        SequenceChange::SetStart(100),
        SequenceChange::SetCache(20),
        SequenceChange::SetCycle(true),
    ]
}

pub fn all_type_change_variants() -> Vec<TypeChange> {
    vec![
        TypeChange::AddValue {
            value: "archived".to_string(),
            position: Some(EnumValuePosition::After("active".to_string())),
        },
        TypeChange::RenameValue {
            from: "draft".to_string(),
            to: "pending".to_string(),
        },
    ]
}

pub fn all_domain_change_variants() -> Vec<DomainChange> {
    vec![
        DomainChange::SetDefault(Some(Expr::Literal(Literal::String("unknown".to_string())))),
        DomainChange::SetNotNull(false),
        DomainChange::AddConstraint {
            name: Some(ident("email_domain_check")),
            check: Expr::Raw("VALUE <> ''".to_string()),
        },
        DomainChange::DropConstraint(ident("email_domain_old_check")),
    ]
}

pub fn all_diffop_variants() -> Vec<DiffOp> {
    let table_name = qualified(Some("public"), "users");

    vec![
        DiffOp::CreateTable(sample_table("users")),
        DiffOp::DropTable(table_name.clone()),
        DiffOp::RenameTable {
            from: table_name.clone(),
            to: qualified(Some("public"), "app_users"),
        },
        DiffOp::AddColumn {
            table: table_name.clone(),
            column: Box::new(sample_column("nickname")),
            position: Some(ColumnPosition::After(ident("email"))),
        },
        DiffOp::DropColumn {
            table: table_name.clone(),
            column: ident("legacy_code"),
        },
        DiffOp::AlterColumn {
            table: table_name.clone(),
            column: ident("email"),
            changes: all_column_change_variants(),
        },
        DiffOp::RenameColumn {
            table: table_name.clone(),
            from: ident("full_name"),
            to: ident("display_name"),
        },
        DiffOp::AddIndex(sample_index()),
        DiffOp::DropIndex {
            owner: IndexOwner::Table(table_name.clone()),
            name: ident("users_email_idx"),
        },
        DiffOp::RenameIndex {
            owner: IndexOwner::Table(table_name.clone()),
            from: ident("users_email_idx"),
            to: ident("users_email_unique_idx"),
        },
        DiffOp::AddForeignKey {
            table: table_name.clone(),
            fk: ForeignKey {
                name: Some(ident("users_org_fk")),
                columns: vec![ident("org_id")],
                referenced_table: qualified(Some("public"), "organizations"),
                referenced_columns: vec![ident("id")],
                on_delete: Some(ForeignKeyAction::Cascade),
                on_update: Some(ForeignKeyAction::NoAction),
                deferrable: None,
                extra: BTreeMap::new(),
            },
        },
        DiffOp::DropForeignKey {
            table: table_name.clone(),
            name: ident("users_org_fk"),
        },
        DiffOp::AddCheck {
            table: table_name.clone(),
            check: CheckConstraint {
                name: Some(ident("users_age_check")),
                expr: Expr::Raw("age >= 0".to_string()),
                no_inherit: false,
            },
        },
        DiffOp::DropCheck {
            table: table_name.clone(),
            name: ident("users_age_check"),
        },
        DiffOp::AddExclusion {
            table: table_name.clone(),
            exclusion: ExclusionConstraint {
                name: Some(ident("users_room_excl")),
                index_method: "gist".to_string(),
                elements: vec![ExclusionElement {
                    expr: Expr::Ident(ident("room_id")),
                    operator: "=".to_string(),
                    opclass: None,
                    order: Some(SortOrder::Asc),
                    nulls: Some(NullsOrder::Last),
                }],
                where_clause: None,
                deferrable: None,
            },
        },
        DiffOp::DropExclusion {
            table: table_name.clone(),
            name: ident("users_room_excl"),
        },
        DiffOp::SetPrimaryKey {
            table: table_name.clone(),
            pk: PrimaryKey {
                name: Some(ident("users_pkey")),
                columns: vec![ident("id")],
            },
        },
        DiffOp::DropPrimaryKey {
            table: table_name.clone(),
        },
        DiffOp::AddPartition {
            table: table_name.clone(),
            partition: Partition {
                strategy: PartitionStrategy::List,
                columns: vec![ident("tenant_id")],
                partitions: vec![PartitionElement {
                    name: ident("users_tenant_a"),
                    bound: Some(PartitionBound::In(vec![Expr::Literal(Literal::String(
                        "tenant_a".to_string(),
                    ))])),
                    extra: BTreeMap::new(),
                }],
            },
        },
        DiffOp::DropPartition {
            table: table_name.clone(),
            name: ident("users_tenant_a"),
        },
        DiffOp::CreateView(sample_view()),
        DiffOp::DropView(qualified(Some("public"), "active_users")),
        DiffOp::CreateMaterializedView(sample_materialized_view()),
        DiffOp::DropMaterializedView(qualified(Some("public"), "active_users_mv")),
        DiffOp::CreateSequence(sample_sequence()),
        DiffOp::DropSequence(qualified(Some("public"), "users_id_seq")),
        DiffOp::AlterSequence {
            name: qualified(Some("public"), "users_id_seq"),
            changes: all_sequence_change_variants(),
        },
        DiffOp::CreateTrigger(sample_trigger()),
        DiffOp::DropTrigger {
            name: qualified(Some("public"), "users_set_updated_at"),
            table: Some(table_name.clone()),
        },
        DiffOp::CreateFunction(sample_function()),
        DiffOp::DropFunction(qualified(Some("public"), "set_updated_at")),
        DiffOp::CreateType(sample_type()),
        DiffOp::DropType(qualified(Some("public"), "status")),
        DiffOp::AlterType {
            name: qualified(Some("public"), "status"),
            change: TypeChange::AddValue {
                value: "archived".to_string(),
                position: Some(EnumValuePosition::Before("active".to_string())),
            },
        },
        DiffOp::CreateDomain(sample_domain()),
        DiffOp::DropDomain(qualified(Some("public"), "email_domain")),
        DiffOp::AlterDomain {
            name: qualified(Some("public"), "email_domain"),
            change: DomainChange::AddConstraint {
                name: Some(ident("email_domain_check")),
                check: Expr::Raw("VALUE <> ''".to_string()),
            },
        },
        DiffOp::CreateExtension(sample_extension()),
        DiffOp::DropExtension(qualified(None, "pg_trgm")),
        DiffOp::CreateSchema(sample_schema()),
        DiffOp::DropSchema(qualified(None, "reporting")),
        DiffOp::SetComment(sample_comment()),
        DiffOp::DropComment {
            target: CommentTarget::Column {
                table: table_name.clone(),
                column: ident("email"),
            },
        },
        DiffOp::Grant(sample_privilege()),
        DiffOp::Revoke(sample_privilege()),
        DiffOp::CreatePolicy(sample_policy()),
        DiffOp::DropPolicy {
            name: ident("users_isolation"),
            table: table_name.clone(),
        },
        DiffOp::AlterTableOptions {
            table: table_name,
            options: TableOptions {
                extra: BTreeMap::from([(String::from("autovacuum_enabled"), Value::Bool(true))]),
            },
        },
    ]
}

pub fn diffop_variant_tag(op: &DiffOp) -> &'static str {
    match op {
        DiffOp::CreateTable(_) => "CreateTable",
        DiffOp::DropTable(_) => "DropTable",
        DiffOp::RenameTable { .. } => "RenameTable",
        DiffOp::AddColumn { .. } => "AddColumn",
        DiffOp::DropColumn { .. } => "DropColumn",
        DiffOp::AlterColumn { .. } => "AlterColumn",
        DiffOp::RenameColumn { .. } => "RenameColumn",
        DiffOp::AddIndex(_) => "AddIndex",
        DiffOp::DropIndex { .. } => "DropIndex",
        DiffOp::RenameIndex { .. } => "RenameIndex",
        DiffOp::AddForeignKey { .. } => "AddForeignKey",
        DiffOp::DropForeignKey { .. } => "DropForeignKey",
        DiffOp::AddCheck { .. } => "AddCheck",
        DiffOp::DropCheck { .. } => "DropCheck",
        DiffOp::AddExclusion { .. } => "AddExclusion",
        DiffOp::DropExclusion { .. } => "DropExclusion",
        DiffOp::SetPrimaryKey { .. } => "SetPrimaryKey",
        DiffOp::DropPrimaryKey { .. } => "DropPrimaryKey",
        DiffOp::AddPartition { .. } => "AddPartition",
        DiffOp::DropPartition { .. } => "DropPartition",
        DiffOp::CreateView(_) => "CreateView",
        DiffOp::DropView(_) => "DropView",
        DiffOp::CreateMaterializedView(_) => "CreateMaterializedView",
        DiffOp::DropMaterializedView(_) => "DropMaterializedView",
        DiffOp::CreateSequence(_) => "CreateSequence",
        DiffOp::DropSequence(_) => "DropSequence",
        DiffOp::AlterSequence { .. } => "AlterSequence",
        DiffOp::CreateTrigger(_) => "CreateTrigger",
        DiffOp::DropTrigger { .. } => "DropTrigger",
        DiffOp::CreateFunction(_) => "CreateFunction",
        DiffOp::DropFunction(_) => "DropFunction",
        DiffOp::CreateType(_) => "CreateType",
        DiffOp::DropType(_) => "DropType",
        DiffOp::AlterType { .. } => "AlterType",
        DiffOp::CreateDomain(_) => "CreateDomain",
        DiffOp::DropDomain(_) => "DropDomain",
        DiffOp::AlterDomain { .. } => "AlterDomain",
        DiffOp::CreateExtension(_) => "CreateExtension",
        DiffOp::DropExtension(_) => "DropExtension",
        DiffOp::CreateSchema(_) => "CreateSchema",
        DiffOp::DropSchema(_) => "DropSchema",
        DiffOp::SetComment(_) => "SetComment",
        DiffOp::DropComment { .. } => "DropComment",
        DiffOp::Grant(_) => "Grant",
        DiffOp::Revoke(_) => "Revoke",
        DiffOp::CreatePolicy(_) => "CreatePolicy",
        DiffOp::DropPolicy { .. } => "DropPolicy",
        DiffOp::AlterTableOptions { .. } => "AlterTableOptions",
    }
}

pub fn column_change_variant_tag(change: &ColumnChange) -> &'static str {
    match change {
        ColumnChange::SetType(_) => "SetType",
        ColumnChange::SetNotNull(_) => "SetNotNull",
        ColumnChange::SetDefault(_) => "SetDefault",
        ColumnChange::SetIdentity(_) => "SetIdentity",
        ColumnChange::SetGenerated(_) => "SetGenerated",
        ColumnChange::SetCollation(_) => "SetCollation",
    }
}

pub fn sequence_change_variant_tag(change: &SequenceChange) -> &'static str {
    match change {
        SequenceChange::SetType(_) => "SetType",
        SequenceChange::SetIncrement(_) => "SetIncrement",
        SequenceChange::SetMinValue(_) => "SetMinValue",
        SequenceChange::SetMaxValue(_) => "SetMaxValue",
        SequenceChange::SetStart(_) => "SetStart",
        SequenceChange::SetCache(_) => "SetCache",
        SequenceChange::SetCycle(_) => "SetCycle",
    }
}

pub fn type_change_variant_tag(change: &TypeChange) -> &'static str {
    match change {
        TypeChange::AddValue { .. } => "AddValue",
        TypeChange::RenameValue { .. } => "RenameValue",
    }
}

pub fn domain_change_variant_tag(change: &DomainChange) -> &'static str {
    match change {
        DomainChange::SetDefault(_) => "SetDefault",
        DomainChange::SetNotNull(_) => "SetNotNull",
        DomainChange::AddConstraint { .. } => "AddConstraint",
        DomainChange::DropConstraint(_) => "DropConstraint",
    }
}
