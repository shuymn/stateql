use std::collections::BTreeMap;

use stateql_core::{
    Column, Comment, CommentTarget, ConnectionConfig, DataType, Dialect, Domain, Error,
    ExecutionError, Expr, Extension, Function, FunctionParam, FunctionSecurity, GenerateError,
    Ident, IndexColumn, IndexDef, IndexOwner, Policy, PolicyCommand, Privilege, PrivilegeObject,
    QualifiedName, SchemaDef, SchemaObject, Sequence, Table, Trigger, TriggerEvent, TriggerForEach,
    TriggerTiming, TypeDef, TypeKind, View,
};
use stateql_dialect_sqlite::SqliteDialect;

#[test]
fn export_roundtrip_is_idempotent_for_table_sql() {
    let dialect = SqliteDialect;
    let exported_sql = "CREATE TABLE users (id integer PRIMARY KEY, name text NOT NULL) STRICT;";

    let first = canonical_export_sql(&dialect, exported_sql);
    let second = canonical_export_sql(&dialect, &first);

    assert_eq!(first, second);
    assert!(first.contains("STRICT"));
}

#[test]
fn to_sql_supports_sqlite_variants_and_rejects_unsupported_variants() {
    let dialect = SqliteDialect;

    let supported_objects = supported_objects();
    let supported_sql = supported_objects
        .iter()
        .map(|object| {
            let sql = dialect
                .to_sql(object)
                .expect("supported sqlite objects must render to SQL");
            assert!(
                !sql.trim().is_empty(),
                "supported sqlite object produced empty SQL"
            );
            sql
        })
        .collect::<Vec<_>>();

    let connection = in_memory_connection();
    let adapter = dialect
        .connect(&connection)
        .expect("sqlite connect should succeed for in-memory database");

    for sql in &supported_sql {
        adapter.execute(sql).unwrap_or_else(|error| {
            panic!("rendered SQL must execute successfully: {sql}\n{error}")
        });
    }

    for object in unsupported_objects() {
        let error = dialect
            .to_sql(&object)
            .expect_err("unsupported sqlite object must return an explicit error");
        match error {
            Error::Generate(GenerateError::UnsupportedDiffOp { .. }) => {}
            other => panic!("expected generate unsupported error, got {other:?}"),
        }
    }
}

#[test]
fn connect_rejects_sqlite_versions_below_3_35() {
    let dialect = SqliteDialect;
    let mut connection = in_memory_connection();
    connection
        .extra
        .insert("sqlite.server_version".to_string(), "3.34.9".to_string());

    let error = match dialect.connect(&connection) {
        Ok(_) => panic!("versions below 3.35 must be rejected"),
        Err(error) => error,
    };

    let source_message = match error {
        Error::Execute(ExecutionError::StatementFailed { source, .. }) => source.to_string(),
        other => panic!("expected execution error, got: {other:?}"),
    };

    assert!(
        source_message.contains("3.35+"),
        "expected minimum-version error, got: {source_message}"
    );
}

fn canonical_export_sql(dialect: &SqliteDialect, sql: &str) -> String {
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

fn in_memory_connection() -> ConnectionConfig {
    ConnectionConfig {
        host: None,
        port: None,
        user: None,
        password: None,
        database: ":memory:".to_string(),
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
    table.columns.push(Column {
        name: Ident::unquoted("id"),
        data_type: DataType::Integer,
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
        name: qualified(None, "users_view"),
        columns: vec![Ident::unquoted("id")],
        query: "SELECT id FROM users".to_string(),
        check_option: None,
        security: None,
        renamed_from: None,
    };

    let index = IndexDef {
        name: Some(Ident::unquoted("idx_users_id")),
        owner: IndexOwner::Table(qualified(None, "users")),
        columns: vec![IndexColumn {
            expr: Expr::Ident(Ident::unquoted("id")),
        }],
        unique: false,
        method: None,
        where_clause: None,
        concurrent: false,
        extra: BTreeMap::new(),
    };

    let trigger = Trigger {
        name: qualified(None, "users_insert_log"),
        table: qualified(None, "users"),
        timing: TriggerTiming::After,
        events: vec![TriggerEvent::Insert],
        for_each: TriggerForEach::Row,
        when_clause: None,
        body: "BEGIN SELECT 1; END".to_string(),
    };

    vec![
        SchemaObject::Table(table),
        SchemaObject::View(view),
        SchemaObject::Index(index),
        SchemaObject::Trigger(trigger),
    ]
}

fn unsupported_objects() -> Vec<SchemaObject> {
    vec![
        SchemaObject::Sequence(Sequence {
            name: qualified(None, "seq"),
            data_type: None,
            increment: None,
            min_value: None,
            max_value: None,
            start: None,
            cache: None,
            cycle: false,
            owned_by: None,
        }),
        SchemaObject::Function(Function {
            name: qualified(None, "f"),
            params: vec![FunctionParam {
                name: Some(Ident::unquoted("arg")),
                data_type: DataType::Integer,
                mode: None,
                default: None,
            }],
            return_type: Some(DataType::Integer),
            language: "sql".to_string(),
            body: "SELECT 1".to_string(),
            volatility: None,
            security: Some(FunctionSecurity::Invoker),
        }),
        SchemaObject::Type(TypeDef {
            name: qualified(None, "status"),
            kind: TypeKind::Enum {
                labels: vec!["active".to_string()],
            },
        }),
        SchemaObject::Domain(Domain {
            name: qualified(None, "email_domain"),
            data_type: DataType::Text,
            default: None,
            not_null: false,
            checks: Vec::new(),
        }),
        SchemaObject::Extension(Extension {
            name: Ident::unquoted("fts5"),
            schema: None,
            version: None,
        }),
        SchemaObject::Schema(SchemaDef {
            name: Ident::unquoted("main"),
        }),
        SchemaObject::Comment(Comment {
            target: CommentTarget::Table(qualified(None, "users")),
            text: Some("comment".to_string()),
        }),
        SchemaObject::Privilege(Privilege {
            operations: vec![stateql_core::PrivilegeOp::Select],
            on: PrivilegeObject::Table(qualified(None, "users")),
            grantee: Ident::unquoted("app"),
            with_grant_option: false,
        }),
        SchemaObject::Policy(Policy {
            name: Ident::unquoted("users_policy"),
            table: qualified(None, "users"),
            command: Some(PolicyCommand::Select),
            using_expr: Some(Expr::Raw("1 = 1".to_string())),
            check_expr: Some(Expr::Raw("1 = 1".to_string())),
            roles: vec![Ident::unquoted("app")],
            permissive: true,
        }),
    ]
}
