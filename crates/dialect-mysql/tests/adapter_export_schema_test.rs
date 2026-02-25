use std::collections::BTreeMap;

use stateql_core::{ConnectionConfig, Dialect};
use stateql_dialect_mysql::{MysqlDialect, lower_case_table_names_query, table_names_query};

#[test]
fn export_queries_keep_view_filter_without_order_by_and_lower_case_variable_probe() {
    let table_names = table_names_query();
    let lower_case_variable = lower_case_table_names_query();

    assert!(table_names.contains("SHOW FULL TABLES"));
    assert!(table_names.contains("Table_Type != 'VIEW'"));
    assert!(!table_names.to_ascii_uppercase().contains("ORDER BY"));
    assert!(lower_case_variable.contains("lower_case_table_names"));
}

#[test]
#[ignore = "requires mysql container runtime"]
fn export_schema_smoke_test_with_container_runtime() {
    if std::env::var("STATEQL_MYSQL_ENABLE_IGNORED").as_deref() != Ok("1") {
        return;
    }

    let host = std::env::var("STATEQL_MYSQL_HOST").unwrap_or_else(|_| "127.0.0.1".to_string());
    let port = std::env::var("STATEQL_MYSQL_PORT")
        .ok()
        .and_then(|raw| raw.parse::<u16>().ok())
        .unwrap_or(3306);
    let user = std::env::var("STATEQL_MYSQL_USER").unwrap_or_else(|_| "root".to_string());
    let password = std::env::var("STATEQL_MYSQL_PASSWORD").unwrap_or_default();
    let database =
        std::env::var("STATEQL_MYSQL_DATABASE").unwrap_or_else(|_| "stateql".to_string());

    let connection = ConnectionConfig {
        host: Some(host),
        port: Some(port),
        user: Some(user),
        password: Some(password),
        database,
        socket: None,
        extra: BTreeMap::new(),
    };

    let dialect = MysqlDialect;
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
