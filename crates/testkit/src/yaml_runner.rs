use std::collections::BTreeMap;

use serde::Deserialize;
use stateql_core::{DatabaseAdapter, Dialect, ParseError, Result, SourceLocation};

const TESTCASE_SOURCE_LABEL: &str = "yaml testcase";

#[derive(Debug, Clone, PartialEq, Eq, Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct TestCase {
    pub current: String,
    pub desired: String,
    pub up: Option<String>,
    pub down: Option<String>,
    pub error: Option<String>,
    pub min_version: Option<String>,
    pub max_version: Option<String>,
    pub flavor: Option<String>,
    /// Runner rule (validated in Task 39/40): `None` resolves to `false` at execution time.
    pub enable_drop: Option<bool>,
    pub offline: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TestResult {
    Passed,
    Skipped(String),
    Failed(String),
}

pub fn load_test_cases_from_str(yaml: &str) -> Result<BTreeMap<String, TestCase>> {
    serde_yaml::from_str(yaml).map_err(|source| parse_yaml_error(yaml, source))
}

pub fn matches_flavor(requirement: Option<&str>, current_flavor: &str) -> bool {
    let Some(requirement) = requirement.map(str::trim).filter(|value| !value.is_empty()) else {
        return true;
    };

    if let Some(excluded_flavor) = requirement.strip_prefix('!') {
        return excluded_flavor != current_flavor;
    }

    requirement == current_flavor
}

pub fn run_offline_test(dialect: &dyn Dialect, test: &TestCase) -> TestResult {
    run_with_flavor_expectation(test, dialect.name(), || {
        run_offline_test_impl(dialect, test)
    })
}

pub fn run_online_test(
    dialect: &dyn Dialect,
    adapter: &mut dyn DatabaseAdapter,
    test: &TestCase,
) -> TestResult {
    run_with_flavor_expectation(test, dialect.name(), || {
        run_online_test_impl(dialect, adapter, test)
    })
}

fn run_with_flavor_expectation(
    test: &TestCase,
    current_flavor: &str,
    execute: impl FnOnce() -> Result<()>,
) -> TestResult {
    let flavor_requirement = test.flavor.as_deref();
    let expect_failure = !matches_flavor(flavor_requirement, current_flavor);
    let execution_result = execute();

    if expect_failure {
        return match execution_result {
            Err(_) => TestResult::Skipped(format!(
                "Correctly fails on non-matching flavor (requires '{}', running on '{}')",
                flavor_requirement.unwrap_or_default(),
                current_flavor
            )),
            Ok(()) => TestResult::Failed(format!(
                "Test passed but flavor '{}' does not match current flavor '{}'",
                flavor_requirement.unwrap_or_default(),
                current_flavor
            )),
        };
    }

    match execution_result {
        Ok(()) => TestResult::Passed,
        Err(error) => TestResult::Failed(error.to_string()),
    }
}

fn run_offline_test_impl(dialect: &dyn Dialect, test: &TestCase) -> Result<()> {
    let _ = dialect.parse(&test.current)?;
    let _ = dialect.parse(&test.desired)?;
    Ok(())
}

fn run_online_test_impl(
    dialect: &dyn Dialect,
    adapter: &mut dyn DatabaseAdapter,
    test: &TestCase,
) -> Result<()> {
    let _ = adapter.server_version()?;
    run_offline_test_impl(dialect, test)
}

fn parse_yaml_error(yaml: &str, source: serde_yaml::Error) -> stateql_core::Error {
    let source_location = source.location().map(|location| SourceLocation {
        line: location.line(),
        column: Some(location.column()),
    });

    ParseError::StatementConversion {
        statement_index: 0,
        source_sql: source_sql_excerpt(yaml),
        source_location,
        source: Box::new(source),
    }
    .into()
}

fn source_sql_excerpt(yaml: &str) -> String {
    let trimmed = yaml.trim();
    if trimmed.is_empty() {
        return TESTCASE_SOURCE_LABEL.to_string();
    }

    const MAX_CHARS: usize = 256;
    if trimmed.chars().count() <= MAX_CHARS {
        return trimmed.to_string();
    }

    let mut excerpt: String = trimmed.chars().take(MAX_CHARS).collect();
    excerpt.push_str("...");
    excerpt
}
