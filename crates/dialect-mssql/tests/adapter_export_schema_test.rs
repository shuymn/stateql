use std::collections::BTreeMap;

use stateql_core::{ConnectionConfig, Dialect};
use stateql_dialect_mssql::{
    MssqlDialect, current_schema_query, server_version_query, table_names_query,
};

#[test]
fn export_queries_cover_schema_names_and_table_catalog() {
    let version_query = server_version_query();
    let schema_query = current_schema_query();
    let table_query = table_names_query();

    assert!(version_query.contains("SERVERPROPERTY('ProductVersion')"));
    assert!(schema_query.contains("SCHEMA_NAME()"));
    assert!(table_query.contains("FROM sys.tables"));
}

#[test]
fn connect_with_overrides_exposes_dbo_search_path_and_export_sql() {
    let dialect = MssqlDialect;
    let mut connection = sample_connection();
    connection.extra.insert(
        "mssql.server_version".to_string(),
        "15.0.2000.5".to_string(),
    );
    connection.extra.insert(
        "mssql.export_schema_sql".to_string(),
        "CREATE TABLE [dbo].[users] ([id] BIGINT NOT NULL);".to_string(),
    );

    let adapter = dialect
        .connect(&connection)
        .expect("connect should succeed for override-backed adapter path");

    assert_eq!(adapter.schema_search_path(), vec!["dbo".to_string()]);

    let first = adapter
        .export_schema()
        .expect("export_schema should return configured export SQL");
    let second = adapter
        .export_schema()
        .expect("export_schema should be deterministic");

    assert_eq!(first, second);
    assert!(first.contains("CREATE TABLE"));
}

#[test]
#[ignore = "requires SQL Server container runtime"]
fn export_schema_smoke_test_with_container_runtime() {
    if std::env::var("STATEQL_MSSQL_ENABLE_IGNORED").as_deref() != Ok("1") {
        return;
    }

    let host = std::env::var("STATEQL_MSSQL_HOST").unwrap_or_else(|_| "127.0.0.1".to_string());
    let port = std::env::var("STATEQL_MSSQL_PORT")
        .ok()
        .and_then(|raw| raw.parse::<u16>().ok())
        .unwrap_or(1433);
    let user = std::env::var("STATEQL_MSSQL_USER").unwrap_or_else(|_| "sa".to_string());
    let password =
        std::env::var("STATEQL_MSSQL_PASSWORD").unwrap_or_else(|_| "Passw0rd!".to_string());
    let database = std::env::var("STATEQL_MSSQL_DATABASE").unwrap_or_else(|_| "master".to_string());

    let connection = ConnectionConfig {
        host: Some(host),
        port: Some(port),
        user: Some(user),
        password: Some(password),
        database,
        socket: None,
        extra: BTreeMap::new(),
    };

    let dialect = MssqlDialect;
    let adapter = dialect
        .connect(&connection)
        .expect("connect should succeed for integration runtime");

    let exported = adapter
        .export_schema()
        .expect("export_schema should succeed for integration runtime");
    assert!(
        !exported.trim().is_empty(),
        "exported schema should not be empty"
    );
    assert!(
        exported.contains("CREATE TABLE"),
        "exported schema should include CREATE TABLE statements"
    );
}

fn sample_connection() -> ConnectionConfig {
    ConnectionConfig {
        host: Some("127.0.0.1".to_string()),
        port: Some(1433),
        user: Some("sa".to_string()),
        password: Some("Passw0rd!".to_string()),
        database: "stateql".to_string(),
        socket: None,
        extra: BTreeMap::new(),
    }
}
