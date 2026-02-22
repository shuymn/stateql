#[path = "support/offline_fake_dialect.rs"]
mod offline_fake_dialect;

use std::{
    cell::RefCell,
    collections::{BTreeMap, BTreeSet},
};

use offline_fake_dialect::OfflineFakeDialect;
use stateql_core::{
    ConnectionConfig, DatabaseAdapter, Dialect, DiffError, Result, Transaction, Version,
};
use stateql_testkit::{TestCase, TestResult, run_online_test};

#[test]
fn online_runner_executes_expected_round_trip_flow_with_fake_adapter() {
    let dialect = OfflineFakeDialect::default();
    let mut adapter = FakeOnlineAdapter::new();
    let testcase = TestCase {
        current: String::new(),
        desired: "tables:users".to_string(),
        up: Some("CREATE TABLE users;".to_string()),
        down: Some("DROP TABLE users;".to_string()),
        enable_drop: Some(true),
        ..TestCase::default()
    };

    let result = run_online_test(&dialect, &mut adapter, &testcase);
    assert!(
        matches!(result, TestResult::Passed),
        "online runner should pass a valid round-trip fixture, got: {result:?}"
    );

    assert_eq!(
        adapter.version_calls(),
        1,
        "online runner must check server version once"
    );
    assert_eq!(
        adapter.export_calls(),
        5,
        "online runner must run all export-based checks in the 8-step flow"
    );

    let executed = adapter.executed_sql();
    assert!(
        executed.iter().any(|sql| sql == "CREATE TABLE users;"),
        "online runner must apply forward DDLs"
    );
    assert!(
        executed.iter().any(|sql| sql == "DROP TABLE users;"),
        "online runner must apply reverse DDLs"
    );

    touch_offline_fake_helpers(&dialect);
}

#[test]
fn online_runner_detects_current_schema_idempotency_regression() {
    let dialect = OfflineFakeDialect::default();
    let mut adapter = FakeOnlineAdapter::new().with_export_override("tables:ghost");
    let testcase = TestCase {
        current: String::new(),
        desired: String::new(),
        enable_drop: Some(true),
        ..TestCase::default()
    };

    let result = run_online_test(&dialect, &mut adapter, &testcase);
    match result {
        TestResult::Failed(message) => {
            assert!(
                message.contains("current schema is not idempotent"),
                "expected current idempotency failure, got: {message}"
            );
        }
        _ => panic!("expected online runner failure, got: {result:?}"),
    }
}

#[test]
fn online_runner_honors_expected_error_for_online_failures() {
    let dialect = OfflineFakeDialect::default();
    let mut adapter = FakeOnlineAdapter::new().with_export_error("export failed");
    let testcase = TestCase {
        current: String::new(),
        desired: String::new(),
        up: None,
        down: None,
        error: Some(expected_adapter_error_message("export failed")),
        ..TestCase::default()
    };

    let result = run_online_test(&dialect, &mut adapter, &testcase);
    assert!(
        matches!(result, TestResult::Passed),
        "matching online-path expected error should pass, got: {result:?}"
    );
}

#[test]
fn online_runner_propagates_enable_drop_in_live_online_diff() {
    assert_online_enable_drop_behavior(
        Some(false),
        "",
        "",
        false,
        "enable_drop=false should avoid destructive forward/reverse SQL in live flow",
    );
    assert_online_enable_drop_behavior(
        Some(true),
        "DROP TABLE users;",
        "CREATE TABLE users;",
        true,
        "enable_drop=true should allow drop/create round-trip in live flow",
    );
}

#[test]
fn online_runner_skips_when_server_version_is_outside_range() {
    let dialect = OfflineFakeDialect::default();
    let mut adapter = FakeOnlineAdapter::new().with_version(Version {
        major: 7,
        minor: 5,
        patch: 0,
    });
    let testcase = TestCase {
        current: String::new(),
        desired: String::new(),
        min_version: Some("8.0".to_string()),
        ..TestCase::default()
    };

    let result = run_online_test(&dialect, &mut adapter, &testcase);
    match result {
        TestResult::Skipped(reason) => {
            assert!(
                reason.contains("smaller than min_version"),
                "expected min_version skip reason, got: {reason}"
            );
        }
        _ => panic!("expected version-gated skip, got: {result:?}"),
    }
}

#[test]
#[ignore = "requires postgres container runtime"]
fn online_runner_round_trip_with_postgres_adapter() {
    if std::env::var("STATEQL_POSTGRES_ENABLE_IGNORED").as_deref() != Ok("1") {
        return;
    }

    let dialect = stateql_dialect_postgres::PostgresDialect;
    let mut adapter = dialect
        .connect(&postgres_connection())
        .expect("postgres connect should succeed for online runner test");

    let testcase = TestCase {
        current: String::new(),
        desired: "CREATE TABLE stateql_online_runner_users (id BIGINT);".to_string(),
        up: None,
        down: None,
        enable_drop: Some(false),
        ..TestCase::default()
    };

    let result = run_online_test(&dialect, adapter.as_mut(), &testcase);
    assert!(
        matches!(result, TestResult::Passed),
        "postgres online runner ignored test should pass, got: {result:?}"
    );
}

#[test]
#[ignore = "requires mysql container runtime"]
fn online_runner_round_trip_with_mysql_adapter() {
    if std::env::var("STATEQL_MYSQL_ENABLE_IGNORED").as_deref() != Ok("1") {
        return;
    }

    let dialect = stateql_dialect_mysql::MysqlDialect;
    let mut adapter = dialect
        .connect(&mysql_connection())
        .expect("mysql connect should succeed for online runner test");

    let testcase = TestCase {
        current: String::new(),
        desired: "CREATE TABLE stateql_online_runner_users (id BIGINT);".to_string(),
        up: None,
        down: None,
        enable_drop: Some(false),
        ..TestCase::default()
    };

    let result = run_online_test(&dialect, adapter.as_mut(), &testcase);
    assert!(
        matches!(result, TestResult::Passed),
        "mysql online runner ignored test should pass, got: {result:?}"
    );
}

#[test]
#[ignore = "requires SQL Server container runtime"]
fn online_runner_round_trip_with_mssql_adapter() {
    if std::env::var("STATEQL_MSSQL_ENABLE_IGNORED").as_deref() != Ok("1") {
        return;
    }

    let dialect = stateql_dialect_mssql::MssqlDialect;
    let mut adapter = dialect
        .connect(&mssql_connection())
        .expect("mssql connect should succeed for online runner test");

    let testcase = TestCase {
        current: String::new(),
        desired: "CREATE TABLE stateql_online_runner_users (id BIGINT);".to_string(),
        up: None,
        down: None,
        enable_drop: Some(false),
        ..TestCase::default()
    };

    let result = run_online_test(&dialect, adapter.as_mut(), &testcase);
    assert!(
        matches!(result, TestResult::Passed),
        "mssql online runner ignored test should pass, got: {result:?}"
    );
}

fn assert_online_enable_drop_behavior(
    enable_drop: Option<bool>,
    expected_up: &str,
    expected_down: &str,
    expect_drop_sql: bool,
    message: &str,
) {
    let dialect = OfflineFakeDialect::default();
    let mut adapter = FakeOnlineAdapter::new();
    let testcase = TestCase {
        current: "tables:users".to_string(),
        desired: String::new(),
        up: Some(expected_up.to_string()),
        down: Some(expected_down.to_string()),
        enable_drop,
        ..TestCase::default()
    };

    let result = run_online_test(&dialect, &mut adapter, &testcase);
    assert!(
        matches!(result, TestResult::Passed),
        "{message}: {result:?}"
    );

    let executed = adapter.executed_sql();
    let dropped_users = executed.iter().any(|sql| sql == "DROP TABLE users;");
    assert_eq!(
        dropped_users, expect_drop_sql,
        "{message} (executed_sql={executed:?})"
    );
}

fn expected_adapter_error_message(message: &str) -> String {
    fake_adapter_error(message).to_string()
}

fn fake_adapter_error(message: &str) -> stateql_core::Error {
    DiffError::ObjectComparison {
        target: "fake_online_adapter".to_string(),
        operation: message.to_string(),
    }
    .into()
}

#[derive(Debug, Clone, Default)]
struct FakeAdapterState {
    tables: BTreeSet<String>,
    executed_sql: Vec<String>,
    export_calls: usize,
    version_calls: usize,
}

#[derive(Debug)]
struct FakeOnlineAdapter {
    state: RefCell<FakeAdapterState>,
    version: Version,
    schema_search_path: Vec<String>,
    export_override: Option<String>,
    export_error: Option<String>,
}

impl FakeOnlineAdapter {
    fn new() -> Self {
        Self {
            state: RefCell::new(FakeAdapterState::default()),
            version: Version {
                major: 8,
                minor: 0,
                patch: 0,
            },
            schema_search_path: vec!["public".to_string()],
            export_override: None,
            export_error: None,
        }
    }

    fn with_export_override(mut self, export_sql: &str) -> Self {
        self.export_override = Some(export_sql.to_string());
        self
    }

    fn with_export_error(mut self, message: &str) -> Self {
        self.export_error = Some(message.to_string());
        self
    }

    fn with_version(mut self, version: Version) -> Self {
        self.version = version;
        self
    }

    fn executed_sql(&self) -> Vec<String> {
        self.state.borrow().executed_sql.clone()
    }

    fn export_calls(&self) -> usize {
        self.state.borrow().export_calls
    }

    fn version_calls(&self) -> usize {
        self.state.borrow().version_calls
    }
}

impl DatabaseAdapter for FakeOnlineAdapter {
    fn export_schema(&self) -> Result<String> {
        let mut state = self.state.borrow_mut();
        state.export_calls += 1;

        if let Some(message) = self.export_error.as_deref() {
            return Err(fake_adapter_error(message));
        }

        if let Some(export_sql) = self.export_override.as_ref() {
            return Ok(export_sql.clone());
        }

        if state.tables.is_empty() {
            return Ok(String::new());
        }

        let list = state.tables.iter().cloned().collect::<Vec<_>>().join(",");
        Ok(format!("tables:{list}"))
    }

    fn execute(&self, sql: &str) -> Result<()> {
        let mut state = self.state.borrow_mut();
        state.executed_sql.push(sql.to_string());

        if sql == "COMMIT" || sql == "ROLLBACK" {
            return Ok(());
        }

        if let Some(table) = sql
            .strip_prefix("CREATE TABLE ")
            .and_then(|rest| rest.strip_suffix(';'))
        {
            state.tables.insert(table.trim().to_string());
            return Ok(());
        }

        if let Some(table) = sql
            .strip_prefix("DROP TABLE ")
            .and_then(|rest| rest.strip_suffix(';'))
        {
            state.tables.remove(table.trim());
            return Ok(());
        }

        Err(fake_adapter_error(&format!(
            "unsupported execute SQL: {sql}"
        )))
    }

    fn begin(&mut self) -> Result<Transaction<'_>> {
        Ok(Transaction::new(self))
    }

    fn schema_search_path(&self) -> Vec<String> {
        self.schema_search_path.clone()
    }

    fn server_version(&self) -> Result<Version> {
        self.state.borrow_mut().version_calls += 1;
        Ok(self.version.clone())
    }
}

fn postgres_connection() -> ConnectionConfig {
    let host = std::env::var("STATEQL_POSTGRES_HOST").unwrap_or_else(|_| "127.0.0.1".to_string());
    let port = std::env::var("STATEQL_POSTGRES_PORT")
        .ok()
        .and_then(|raw| raw.parse::<u16>().ok())
        .unwrap_or(5432);
    let user = std::env::var("STATEQL_POSTGRES_USER").unwrap_or_else(|_| "postgres".to_string());
    let password = std::env::var("STATEQL_POSTGRES_PASSWORD").unwrap_or_default();
    let database =
        std::env::var("STATEQL_POSTGRES_DATABASE").unwrap_or_else(|_| "postgres".to_string());

    ConnectionConfig {
        host: Some(host),
        port: Some(port),
        user: Some(user),
        password: Some(password),
        database,
        socket: None,
        extra: BTreeMap::new(),
    }
}

fn mysql_connection() -> ConnectionConfig {
    let host = std::env::var("STATEQL_MYSQL_HOST").unwrap_or_else(|_| "127.0.0.1".to_string());
    let port = std::env::var("STATEQL_MYSQL_PORT")
        .ok()
        .and_then(|raw| raw.parse::<u16>().ok())
        .unwrap_or(3306);
    let user = std::env::var("STATEQL_MYSQL_USER").unwrap_or_else(|_| "root".to_string());
    let password = std::env::var("STATEQL_MYSQL_PASSWORD").unwrap_or_default();
    let database =
        std::env::var("STATEQL_MYSQL_DATABASE").unwrap_or_else(|_| "stateql".to_string());

    ConnectionConfig {
        host: Some(host),
        port: Some(port),
        user: Some(user),
        password: Some(password),
        database,
        socket: None,
        extra: BTreeMap::new(),
    }
}

fn mssql_connection() -> ConnectionConfig {
    let host = std::env::var("STATEQL_MSSQL_HOST").unwrap_or_else(|_| "127.0.0.1".to_string());
    let port = std::env::var("STATEQL_MSSQL_PORT")
        .ok()
        .and_then(|raw| raw.parse::<u16>().ok())
        .unwrap_or(1433);
    let user = std::env::var("STATEQL_MSSQL_USER").unwrap_or_else(|_| "sa".to_string());
    let password =
        std::env::var("STATEQL_MSSQL_PASSWORD").unwrap_or_else(|_| "Passw0rd!".to_string());
    let database = std::env::var("STATEQL_MSSQL_DATABASE").unwrap_or_else(|_| "master".to_string());

    ConnectionConfig {
        host: Some(host),
        port: Some(port),
        user: Some(user),
        password: Some(password),
        database,
        socket: None,
        extra: BTreeMap::new(),
    }
}

fn touch_offline_fake_helpers(dialect: &OfflineFakeDialect) {
    let _ = dialect.generated_batches();
    let _ = OfflineFakeDialect::expected_error_message("sample");
}
