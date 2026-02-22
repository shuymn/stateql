use std::collections::BTreeMap;

use stateql_core::{
    Column, Comment, CommentTarget, ConnectionConfig, DataType, Dialect, Domain, Error,
    ExecutionError, Expr, Extension, Function, FunctionParam, GenerateError, Ident, IndexColumn,
    IndexDef, IndexOwner, Policy, PolicyCommand, Privilege, PrivilegeObject, QualifiedName,
    SchemaDef, SchemaObject, Table, Trigger, TriggerEvent, TriggerForEach, TriggerTiming, View,
};
use stateql_dialect_mssql::MssqlDialect;

#[test]
fn export_roundtrip_is_idempotent_for_identity_and_clustered_table_sql() {
    let dialect = MssqlDialect;
    let exported_sql = "\
CREATE TABLE [dbo].[Users] (\
    [Id] BIGINT IDENTITY(1,1) NOT NULL,\
    [Name] NVARCHAR(255) NOT NULL,\
    CONSTRAINT [PK_Users] PRIMARY KEY CLUSTERED ([Id] ASC)\
);";

    let first = canonical_export_sql(&dialect, exported_sql);
    let second = canonical_export_sql(&dialect, &first);

    assert_eq!(first, second);
    let upper = first.to_ascii_uppercase();
    assert!(upper.contains("IDENTITY"));
    assert!(upper.contains("CLUSTERED"));
}

#[test]
fn to_sql_supports_mssql_variants_and_rejects_unsupported_variants() {
    let dialect = MssqlDialect;

    for object in supported_objects() {
        let sql = dialect
            .to_sql(&object)
            .expect("supported mssql objects must render to SQL");
        assert!(
            !sql.trim().is_empty(),
            "supported mssql object produced empty SQL"
        );
    }

    for object in unsupported_objects() {
        let error = dialect
            .to_sql(&object)
            .expect_err("unsupported mssql object must return an explicit error");
        match error {
            Error::Generate(GenerateError::UnsupportedDiffOp { .. }) => {}
            other => panic!("expected generate unsupported error, got {other:?}"),
        }
    }
}

#[test]
fn connect_rejects_sql_server_versions_below_2019() {
    let dialect = MssqlDialect;
    let mut connection = sample_connection();
    connection.extra.insert(
        "mssql.server_version".to_string(),
        "14.0.1000.169".to_string(),
    );

    let error = match dialect.connect(&connection) {
        Ok(_) => panic!("versions below SQL Server 2019 must be rejected"),
        Err(error) => error,
    };

    let source_message = match error {
        Error::Execute(ExecutionError::StatementFailed { source, .. }) => source.to_string(),
        other => panic!("expected execution error, got: {other:?}"),
    };

    assert!(
        source_message.contains("2019+"),
        "expected minimum-version error, got: {source_message}"
    );
}

fn canonical_export_sql(dialect: &MssqlDialect, sql: &str) -> String {
    let mut objects = dialect.parse(sql).expect("parse should succeed");
    for object in &mut objects {
        dialect.normalize(object);
    }

    objects
        .iter()
        .map(|object| dialect.to_sql(object).expect("to_sql should succeed"))
        .collect::<Vec<_>>()
        .join("\n")
}

fn sample_connection() -> ConnectionConfig {
    ConnectionConfig {
        host: Some("127.0.0.1".to_string()),
        port: Some(1433),
        user: Some("sa".to_string()),
        password: Some("stateql_password".to_string()),
        database: "stateql".to_string(),
        socket: None,
        extra: BTreeMap::new(),
    }
}

fn qualified(schema: Option<&str>, name: &str) -> QualifiedName {
    QualifiedName {
        schema: schema.map(Ident::unquoted),
        name: Ident::unquoted(name),
    }
}

fn supported_objects() -> Vec<SchemaObject> {
    let mut table = Table::named("users");
    table.name = qualified(Some("dbo"), "users");
    table.columns.push(Column {
        name: Ident::unquoted("id"),
        data_type: DataType::BigInt,
        not_null: true,
        default: None,
        identity: None,
        generated: None,
        comment: None,
        collation: None,
        renamed_from: None,
        extra: BTreeMap::new(),
    });

    let view = View {
        name: qualified(Some("dbo"), "users_view"),
        columns: vec![Ident::unquoted("id")],
        query: "SELECT id FROM dbo.users".to_string(),
        check_option: None,
        security: None,
        renamed_from: None,
    };

    let index = IndexDef {
        name: Some(Ident::unquoted("ix_users_id")),
        owner: IndexOwner::Table(qualified(Some("dbo"), "users")),
        columns: vec![IndexColumn {
            expr: Expr::Ident(Ident::unquoted("id")),
        }],
        unique: false,
        method: Some("CLUSTERED".to_string()),
        where_clause: None,
        concurrent: false,
        extra: BTreeMap::new(),
    };

    let trigger = Trigger {
        name: qualified(Some("dbo"), "trg_users_audit"),
        table: qualified(Some("dbo"), "users"),
        timing: TriggerTiming::After,
        events: vec![TriggerEvent::Insert],
        for_each: TriggerForEach::Statement,
        when_clause: None,
        body: "INSERT INTO dbo.audit_log(user_id) SELECT id FROM inserted".to_string(),
    };

    let function = Function {
        name: qualified(Some("dbo"), "active_user_count"),
        params: vec![FunctionParam {
            name: Some(Ident::unquoted("status")),
            data_type: DataType::Varchar { length: Some(32) },
            mode: None,
            default: None,
        }],
        return_type: Some(DataType::Integer),
        language: "tsql".to_string(),
        body: "RETURN (SELECT COUNT(*) FROM dbo.users)".to_string(),
        volatility: None,
        security: None,
    };

    let schema = SchemaDef {
        name: Ident::unquoted("app"),
    };

    vec![
        SchemaObject::Table(table),
        SchemaObject::View(view),
        SchemaObject::Index(index),
        SchemaObject::Trigger(trigger),
        SchemaObject::Function(function),
        SchemaObject::Schema(schema),
    ]
}

fn unsupported_objects() -> Vec<SchemaObject> {
    vec![
        SchemaObject::Domain(Domain {
            name: qualified(Some("dbo"), "email_domain"),
            data_type: DataType::Text,
            default: None,
            not_null: false,
            checks: Vec::new(),
        }),
        SchemaObject::Extension(Extension {
            name: Ident::unquoted("example"),
            schema: None,
            version: None,
        }),
        SchemaObject::Comment(Comment {
            target: CommentTarget::Table(qualified(Some("dbo"), "users")),
            text: Some("comment".to_string()),
        }),
        SchemaObject::Privilege(Privilege {
            operations: vec![stateql_core::PrivilegeOp::Select],
            on: PrivilegeObject::Table(qualified(Some("dbo"), "users")),
            grantee: Ident::unquoted("app"),
            with_grant_option: false,
        }),
        SchemaObject::Policy(Policy {
            name: Ident::unquoted("users_policy"),
            table: qualified(Some("dbo"), "users"),
            command: Some(PolicyCommand::Select),
            using_expr: Some(Expr::Raw("1 = 1".to_string())),
            check_expr: Some(Expr::Raw("1 = 1".to_string())),
            roles: vec![Ident::unquoted("app")],
            permissive: true,
        }),
    ]
}
