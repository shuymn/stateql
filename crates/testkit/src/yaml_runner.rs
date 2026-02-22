use std::{collections::BTreeMap, sync::Arc};

use serde::Deserialize;
use stateql_core::{
    DatabaseAdapter, Dialect, DiffConfig, DiffEngine, DiffError, EquivalencePolicy, Expr,
    ParseError, Renderer, Result, SchemaObject, SourceLocation, Statement,
};

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
        evaluate_expected_error(test, run_offline_test_flow(dialect, test))
    })
}

pub fn run_online_test(
    dialect: &dyn Dialect,
    adapter: &mut dyn DatabaseAdapter,
    test: &TestCase,
) -> TestResult {
    run_with_flavor_expectation(test, dialect.name(), || {
        evaluate_expected_error(test, run_online_test_flow(dialect, adapter, test))
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

fn run_offline_test_flow(dialect: &dyn Dialect, test: &TestCase) -> Result<()> {
    validate_direction_expectations(test)?;

    let enable_drop = test.enable_drop.unwrap_or(false);
    let forward = generate_statements(dialect, &test.desired, &test.current, enable_drop)?;
    assert_expected_sql("up", test.up.as_deref(), dialect, &forward)?;

    let reverse = generate_statements(dialect, &test.current, &test.desired, enable_drop)?;
    assert_expected_sql("down", test.down.as_deref(), dialect, &reverse)?;

    Ok(())
}

fn run_online_test_flow(
    dialect: &dyn Dialect,
    adapter: &mut dyn DatabaseAdapter,
    test: &TestCase,
) -> Result<()> {
    let _ = adapter.server_version()?;
    run_offline_test_flow(dialect, test)
}

fn evaluate_expected_error(test: &TestCase, execution_result: Result<()>) -> Result<()> {
    let Some(expected_error) = test.error.as_deref() else {
        return execution_result;
    };

    match execution_result {
        Ok(()) => Err(runner_assertion_error(format!(
            "expected error: {expected_error}, but got no error"
        ))),
        Err(actual_error) => {
            let actual_error = actual_error.to_string();
            if actual_error == expected_error {
                Ok(())
            } else {
                Err(runner_assertion_error(format!(
                    "expected error: {expected_error}, but got: {actual_error}"
                )))
            }
        }
    }
}

fn validate_direction_expectations(test: &TestCase) -> Result<()> {
    match (&test.up, &test.down) {
        (Some(_), Some(_)) | (None, None) => Ok(()),
        _ => Err(runner_assertion_error(
            "`up` and `down` must either both be set or both be omitted",
        )),
    }
}

fn generate_statements(
    dialect: &dyn Dialect,
    desired_sql: &str,
    current_sql: &str,
    enable_drop: bool,
) -> Result<Vec<Statement>> {
    let desired = parse_and_normalize(dialect, desired_sql)?;
    let current = parse_and_normalize(dialect, current_sql)?;
    let diff_config = DiffConfig::new(
        enable_drop,
        Vec::new(),
        Arc::new(DelegatingEquivalencePolicy {
            inner: dialect.equivalence_policy(),
        }),
    );
    let ops = DiffEngine::new().diff(&desired, &current, &diff_config)?;
    dialect.generate_ddl(&ops)
}

fn parse_and_normalize(dialect: &dyn Dialect, sql: &str) -> Result<Vec<SchemaObject>> {
    let mut objects = dialect.parse(sql)?;
    for object in &mut objects {
        dialect.normalize(object);
    }
    Ok(objects)
}

fn assert_expected_sql(
    direction: &str,
    expected: Option<&str>,
    dialect: &dyn Dialect,
    statements: &[Statement],
) -> Result<()> {
    let Some(expected) = expected else {
        return Ok(());
    };

    let actual = Renderer::new(dialect).render(statements);
    if normalize_sql(expected) == normalize_sql(&actual) {
        return Ok(());
    }

    Err(runner_assertion_error(format!(
        "{direction} SQL mismatch; expected:\n{expected}\nactual:\n{}",
        actual.trim()
    )))
}

fn normalize_sql(sql: &str) -> &str {
    sql.trim()
}

fn runner_assertion_error(message: impl Into<String>) -> stateql_core::Error {
    DiffError::ObjectComparison {
        target: "yaml_runner".to_string(),
        operation: message.into(),
    }
    .into()
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

struct DelegatingEquivalencePolicy {
    inner: &'static dyn EquivalencePolicy,
}

impl EquivalencePolicy for DelegatingEquivalencePolicy {
    fn is_equivalent_expr(&self, left: &Expr, right: &Expr) -> bool {
        self.inner.is_equivalent_expr(left, right)
    }

    fn is_equivalent_custom_type(&self, left: &str, right: &str) -> bool {
        self.inner.is_equivalent_custom_type(left, right)
    }
}
