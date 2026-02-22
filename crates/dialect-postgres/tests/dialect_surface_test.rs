use std::collections::BTreeMap;

use stateql_core::{
    Column, ConnectionConfig, DataType, Dialect, Error, ExecutionError, Ident, SchemaObject, Table,
};
use stateql_dialect_postgres::PostgresDialect;

#[test]
fn export_roundtrip_is_idempotent_for_partitioned_tables() {
    let dialect = PostgresDialect;
    let exported_sql = "\
CREATE TABLE events (id integer) PARTITION BY RANGE (id);\n\
CREATE TABLE events_p1 PARTITION OF events FOR VALUES FROM (1) TO (100);";

    let first = canonical_export_sql(&dialect, exported_sql);
    let second = canonical_export_sql(&dialect, &first);

    assert_eq!(first, second);
    assert!(first.contains("PARTITION OF \"events\""));
}

#[test]
fn to_sql_renders_table_objects() {
    let dialect = PostgresDialect;
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

    let sql = dialect
        .to_sql(&SchemaObject::Table(table))
        .expect("to_sql should support table objects");

    assert!(sql.starts_with("CREATE TABLE"));
    assert!(sql.contains("\"id\" bigint NOT NULL"));
}

#[test]
fn connect_rejects_postgres_versions_below_13() {
    let dialect = PostgresDialect;
    let mut extra = BTreeMap::new();
    extra.insert(
        "postgres.server_version".to_string(),
        "12.17 (Debian 12.17-1.pgdg120+1)".to_string(),
    );
    let connection = ConnectionConfig {
        host: None,
        port: None,
        user: None,
        password: None,
        database: "stateql".to_string(),
        socket: None,
        extra,
    };

    let error = match dialect.connect(&connection) {
        Ok(_) => panic!("versions below 13 must be rejected"),
        Err(error) => error,
    };

    let source_message = match error {
        Error::Execute(ExecutionError::StatementFailed { source, .. }) => source.to_string(),
        other => panic!("expected execution error, got: {other:?}"),
    };
    assert!(
        source_message.contains("13+"),
        "expected a minimum-version error, got: {source_message}"
    );
}

fn canonical_export_sql(dialect: &PostgresDialect, sql: &str) -> String {
    dialect
        .parse(sql)
        .expect("parse should succeed")
        .iter()
        .map(|object| dialect.to_sql(object).expect("to_sql should succeed"))
        .collect::<Vec<_>>()
        .join("\n")
}
