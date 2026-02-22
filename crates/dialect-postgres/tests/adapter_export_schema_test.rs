use std::collections::BTreeMap;

use stateql_core::{ConnectionConfig, Dialect};
use stateql_dialect_postgres::{PostgresDialect, parse_search_path, table_names_query};

#[test]
fn parses_search_path_and_excludes_implicit_schemas() {
    let parsed = parse_search_path("\"$user\", public, app, pg_catalog, pg_temp_3");

    assert_eq!(parsed, vec!["public".to_string(), "app".to_string()]);
}

#[test]
fn export_queries_keep_extension_oid_filter_for_tables() {
    let query = table_names_query();

    assert!(
        query.contains(
            "d.classid = (SELECT oid FROM pg_catalog.pg_class WHERE relname = 'pg_class')"
        )
    );
    assert!(query.contains("d.deptype = 'e'"));
    assert!(query.contains("c.relispartition = false"));
}

#[test]
#[ignore = "requires postgres container runtime"]
fn export_schema_smoke_test_with_container_runtime() {
    if std::env::var("STATEQL_POSTGRES_ENABLE_IGNORED").as_deref() != Ok("1") {
        return;
    }

    let host = std::env::var("STATEQL_POSTGRES_HOST").unwrap_or_else(|_| "127.0.0.1".to_string());
    let port = std::env::var("STATEQL_POSTGRES_PORT")
        .ok()
        .and_then(|raw| raw.parse::<u16>().ok())
        .unwrap_or(5432);
    let user = std::env::var("STATEQL_POSTGRES_USER").unwrap_or_else(|_| "postgres".to_string());
    let password = std::env::var("STATEQL_POSTGRES_PASSWORD").unwrap_or_default();
    let database =
        std::env::var("STATEQL_POSTGRES_DATABASE").unwrap_or_else(|_| "postgres".to_string());

    let connection = ConnectionConfig {
        host: Some(host),
        port: Some(port),
        user: Some(user),
        password: Some(password),
        database,
        socket: None,
        extra: BTreeMap::new(),
    };

    let dialect = PostgresDialect;
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
