use std::collections::BTreeMap;

use stateql_core::{ConnectionConfig, Dialect};
use stateql_dialect_sqlite::{SqliteDialect, table_names_query};

#[test]
fn export_schema_is_deterministic_and_covers_table_view_index_trigger() {
    let dialect = SqliteDialect;
    let adapter = dialect
        .connect(&in_memory_connection())
        .expect("connect should succeed");

    adapter
        .execute("CREATE TABLE users (id INTEGER PRIMARY KEY, name TEXT NOT NULL);")
        .expect("create users table");
    adapter
        .execute("CREATE TABLE audit_log (id INTEGER PRIMARY KEY, user_id INTEGER NOT NULL);")
        .expect("create audit table");
    adapter
        .execute("CREATE VIEW users_view AS SELECT id, name FROM users;")
        .expect("create view");
    adapter
        .execute("CREATE INDEX idx_users_name ON users(name);")
        .expect("create index");
    adapter
        .execute(
            "CREATE TRIGGER users_insert_audit AFTER INSERT ON users BEGIN INSERT INTO audit_log(user_id) VALUES (NEW.id); END;",
        )
        .expect("create trigger");

    let first = adapter
        .export_schema()
        .expect("export_schema should return current sqlite schema");
    let second = adapter
        .export_schema()
        .expect("export_schema should be deterministic across repeated calls");

    assert_eq!(first, second);
    assert!(first.contains("CREATE TABLE users"));
    assert!(first.contains("CREATE TABLE audit_log"));
    assert!(first.contains("CREATE VIEW users_view"));
    assert!(first.contains("CREATE INDEX idx_users_name"));
    assert!(first.contains("CREATE TRIGGER users_insert_audit"));

    let table_pos = first.find("CREATE TABLE").expect("table must exist");
    let view_pos = first.find("CREATE VIEW").expect("view must exist");
    let index_pos = first.find("CREATE INDEX").expect("index must exist");
    let trigger_pos = first.find("CREATE TRIGGER").expect("trigger must exist");
    assert!(table_pos < view_pos);
    assert!(view_pos < index_pos);
    assert!(index_pos < trigger_pos);

    let version = adapter
        .server_version()
        .expect("server_version should be available");
    assert!(version.major >= 3);
    assert_eq!(adapter.schema_search_path(), vec!["main".to_string()]);
}

#[test]
fn table_query_excludes_sqlite_internal_tables() {
    let query = table_names_query();

    assert!(query.contains("type = 'table'"));
    assert!(query.contains("tbl_name NOT LIKE 'sqlite_%'"));
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
