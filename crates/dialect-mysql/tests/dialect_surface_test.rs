use std::collections::BTreeMap;

use stateql_core::{
    Column, Comment, CommentTarget, ConnectionConfig, DataType, Dialect, Domain, Error,
    ExecutionError, Expr, Extension, Function, FunctionParam, FunctionSecurity, GenerateError,
    Ident, IndexColumn, IndexDef, IndexOwner, Policy, PolicyCommand, Privilege, PrivilegeObject,
    QualifiedName, SchemaDef, SchemaObject, Table, Trigger, TriggerEvent, TriggerForEach,
    TriggerTiming,
};
use stateql_dialect_mysql::MysqlDialect;

#[test]
fn export_roundtrip_is_idempotent_for_table_sql() {
    let dialect = MysqlDialect;
    let exported_sql = "\
CREATE TABLE `Users` (
  `id` bigint NOT NULL AUTO_INCREMENT,
  `name` varchar(255) NOT NULL,
  PRIMARY KEY (`id`)
) ENGINE=InnoDB;";

    let first = canonical_export_sql(&dialect, exported_sql);
    let second = canonical_export_sql(&dialect, &first);

    assert_eq!(first, second);
    assert!(first.contains("AUTO_INCREMENT"));
}

#[test]
fn to_sql_supports_mysql_variants_and_rejects_unsupported_variants() {
    let dialect = MysqlDialect;

    for object in supported_objects() {
        let sql = dialect
            .to_sql(&object)
            .expect("supported mysql objects must render to SQL");
        assert!(
            !sql.trim().is_empty(),
            "supported mysql object produced empty SQL"
        );
    }

    for object in unsupported_objects() {
        let error = dialect
            .to_sql(&object)
            .expect_err("unsupported mysql object must return an explicit error");
        match error {
            Error::Generate(GenerateError::UnsupportedDiffOp { .. }) => {}
            other => panic!("expected generate unsupported error, got {other:?}"),
        }
    }
}

#[test]
fn connect_rejects_mysql_versions_below_8_0() {
    let dialect = MysqlDialect;
    let mut connection = sample_connection();
    connection
        .extra
        .insert("mysql.server_version".to_string(), "5.7.44".to_string());

    let error = match dialect.connect(&connection) {
        Ok(_) => panic!("versions below 8.0 must be rejected"),
        Err(error) => error,
    };

    let source_message = match error {
        Error::Execute(ExecutionError::StatementFailed { source, .. }) => source.to_string(),
        other => panic!("expected execution error, got: {other:?}"),
    };

    assert!(
        source_message.contains("8.0+"),
        "expected minimum-version error, got: {source_message}"
    );
}

fn canonical_export_sql(dialect: &MysqlDialect, sql: &str) -> String {
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
        port: Some(3306),
        user: Some("root".to_string()),
        password: Some("password".to_string()),
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

    let view = stateql_core::View {
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
        method: Some("BTREE".to_string()),
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
        body: "SET NEW.id = NEW.id".to_string(),
    };

    let function = Function {
        name: qualified(None, "touch_user"),
        params: vec![FunctionParam {
            name: Some(Ident::unquoted("user_id")),
            data_type: DataType::BigInt,
            mode: None,
            default: None,
        }],
        return_type: Some(DataType::BigInt),
        language: "SQL".to_string(),
        body: "SELECT user_id".to_string(),
        volatility: None,
        security: Some(FunctionSecurity::Invoker),
    };

    vec![
        SchemaObject::Table(table),
        SchemaObject::View(view),
        SchemaObject::Index(index),
        SchemaObject::Trigger(trigger),
        SchemaObject::Function(function),
    ]
}

fn unsupported_objects() -> Vec<SchemaObject> {
    vec![
        SchemaObject::Domain(Domain {
            name: qualified(None, "email_domain"),
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
        SchemaObject::Schema(SchemaDef {
            name: Ident::unquoted("app"),
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
