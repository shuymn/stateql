use std::io::Error as IoError;

use stateql_core::{
    ConnectionConfig, DatabaseAdapter, Dialect, DiffOp, Ident, ParseError, Result, SchemaObject,
    Statement, Transaction, Version,
};
use stateql_testkit::{TestCase, TestResult, matches_flavor, run_offline_test, run_online_test};

fn test_error(message: &str) -> stateql_core::Error {
    ParseError::StatementConversion {
        statement_index: 0,
        source_sql: message.to_string(),
        source_location: None,
        source: Box::new(IoError::other(message.to_string())),
    }
    .into()
}

#[derive(Debug)]
struct FakeDialect {
    name: &'static str,
    parse_should_fail: bool,
}

impl Dialect for FakeDialect {
    fn name(&self) -> &str {
        self.name
    }

    fn parse(&self, _sql: &str) -> Result<Vec<SchemaObject>> {
        if self.parse_should_fail {
            return Err(test_error("parse failure"));
        }
        Ok(Vec::new())
    }

    fn generate_ddl(&self, _ops: &[DiffOp]) -> Result<Vec<Statement>> {
        Ok(Vec::new())
    }

    fn to_sql(&self, _obj: &SchemaObject) -> Result<String> {
        Ok(String::new())
    }

    fn normalize(&self, _obj: &mut SchemaObject) {}

    fn quote_ident(&self, ident: &Ident) -> String {
        ident.value.clone()
    }

    fn connect(&self, _config: &ConnectionConfig) -> Result<Box<dyn DatabaseAdapter>> {
        Err(test_error("connect not available"))
    }
}

struct FakeAdapter;

impl DatabaseAdapter for FakeAdapter {
    fn export_schema(&self) -> Result<String> {
        Ok(String::new())
    }

    fn execute(&self, _sql: &str) -> Result<()> {
        Ok(())
    }

    fn begin(&mut self) -> Result<Transaction<'_>> {
        Ok(Transaction::new(self))
    }

    fn schema_search_path(&self) -> Vec<String> {
        Vec::new()
    }

    fn server_version(&self) -> Result<Version> {
        Ok(Version {
            major: 8,
            minor: 0,
            patch: 0,
        })
    }
}

fn case_with_flavor(flavor: &str) -> TestCase {
    TestCase {
        current: "CREATE TABLE users (id INT);".to_string(),
        desired: "CREATE TABLE users (id INT, name TEXT);".to_string(),
        flavor: Some(flavor.to_string()),
        ..TestCase::default()
    }
}

#[test]
fn flavor_matcher_supports_positive_and_negative_requirements() {
    assert!(matches_flavor(Some("mysql"), "mysql"));
    assert!(!matches_flavor(Some("mysql"), "mariadb"));
    assert!(matches_flavor(Some("!tidb"), "mysql"));
    assert!(!matches_flavor(Some("!tidb"), "tidb"));
}

#[test]
fn offline_runner_skips_when_mismatch_fails_and_fails_when_mismatch_passes() {
    let mismatch = case_with_flavor("mysql");

    let failing_dialect = FakeDialect {
        name: "postgres",
        parse_should_fail: true,
    };
    let passing_dialect = FakeDialect {
        name: "postgres",
        parse_should_fail: false,
    };

    let skipped = run_offline_test(&failing_dialect, &mismatch);
    assert!(
        matches!(skipped, TestResult::Skipped(_)),
        "flavor mismatch + failing execution must become skip"
    );

    let failed = run_offline_test(&passing_dialect, &mismatch);
    assert!(
        matches!(failed, TestResult::Failed(_)),
        "flavor mismatch + passing execution must fail annotation validation"
    );
}

#[test]
fn online_runner_applies_flavor_matching_for_negative_requirements() {
    let mismatch = case_with_flavor("!tidb");
    let mut adapter = FakeAdapter;

    let failing_dialect = FakeDialect {
        name: "tidb",
        parse_should_fail: true,
    };
    let passing_dialect = FakeDialect {
        name: "tidb",
        parse_should_fail: false,
    };

    let skipped = run_online_test(&failing_dialect, &mut adapter, &mismatch);
    assert!(
        matches!(skipped, TestResult::Skipped(_)),
        "negative mismatch + failing execution must become skip"
    );

    let failed = run_online_test(&passing_dialect, &mut adapter, &mismatch);
    assert!(
        matches!(failed, TestResult::Failed(_)),
        "negative mismatch + passing execution must fail annotation validation"
    );
}
