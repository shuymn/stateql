use std::{cmp::Ordering, collections::BTreeMap, sync::Arc};

use serde::Deserialize;
use stateql_core::{
    DatabaseAdapter, Dialect, DiffConfig, DiffEngine, DiffError, EquivalencePolicy, Executor, Expr,
    ParseError, Renderer, Result, SchemaObject, SourceLocation, Statement, Version,
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

#[derive(Debug)]
enum RunnerOutcome {
    Executed(Result<()>),
    Skipped(String),
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
        RunnerOutcome::Executed(evaluate_expected_error(
            test,
            run_offline_test_flow(dialect, test),
        ))
    })
}

pub fn run_online_test(
    dialect: &dyn Dialect,
    adapter: &mut dyn DatabaseAdapter,
    test: &TestCase,
) -> TestResult {
    run_with_flavor_expectation(test, dialect.name(), || match evaluate_online_version_gate(
        adapter, test,
    ) {
        Ok(Some(skip_reason)) => RunnerOutcome::Skipped(skip_reason),
        Ok(None) => RunnerOutcome::Executed(evaluate_expected_error(
            test,
            run_online_test_flow(dialect, adapter, test),
        )),
        Err(error) => RunnerOutcome::Executed(Err(error)),
    })
}

fn run_with_flavor_expectation(
    test: &TestCase,
    current_flavor: &str,
    execute: impl FnOnce() -> RunnerOutcome,
) -> TestResult {
    let flavor_requirement = test.flavor.as_deref();
    let expect_failure = !matches_flavor(flavor_requirement, current_flavor);

    match execute() {
        RunnerOutcome::Skipped(reason) => TestResult::Skipped(reason),
        RunnerOutcome::Executed(execution_result) => {
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
    validate_direction_expectations(test)?;

    let enable_drop = test.enable_drop.unwrap_or(false);

    // 1) Apply current schema.
    let seed = generate_statements_with_search_path(
        dialect,
        &test.current,
        "",
        enable_drop,
        adapter.schema_search_path(),
    )?;
    apply_statements(adapter, &seed)?;

    // 2) Verify idempotency of current schema.
    assert_online_idempotency(
        dialect,
        adapter,
        &test.current,
        enable_drop,
        "current schema",
    )?;

    // 3) Generate current -> desired and validate expected up SQL.
    let forward = generate_statements_from_export(dialect, adapter, &test.desired, enable_drop)?;
    assert_expected_sql("up", test.up.as_deref(), dialect, &forward)?;

    // 4) Apply generated forward DDLs.
    apply_statements(adapter, &forward)?;

    // 5) Verify idempotency of desired schema.
    assert_online_idempotency(
        dialect,
        adapter,
        &test.desired,
        enable_drop,
        "desired schema",
    )?;

    // 6) Generate desired -> current and validate expected down SQL.
    let reverse = generate_statements_from_export(dialect, adapter, &test.current, enable_drop)?;
    assert_expected_sql("down", test.down.as_deref(), dialect, &reverse)?;

    // 7) Apply generated reverse DDLs.
    apply_statements(adapter, &reverse)?;

    // 8) Verify idempotency after reverse.
    assert_online_idempotency(
        dialect,
        adapter,
        &test.current,
        enable_drop,
        "current schema after reverse",
    )?;

    Ok(())
}

fn evaluate_online_version_gate(
    adapter: &dyn DatabaseAdapter,
    test: &TestCase,
) -> Result<Option<String>> {
    let version = adapter.server_version()?;
    version_skip_reason(test, &version)
}

fn version_skip_reason(test: &TestCase, version: &Version) -> Result<Option<String>> {
    let rendered_version = format_version(version);

    if let Some(min_version) = normalized_version_requirement(test.min_version.as_deref())
        && compare_version_against_requirement(version, min_version)? == Ordering::Less
    {
        return Ok(Some(format!(
            "Version '{rendered_version}' is smaller than min_version '{min_version}'"
        )));
    }

    if let Some(max_version) = normalized_version_requirement(test.max_version.as_deref())
        && compare_version_against_requirement(version, max_version)? == Ordering::Greater
    {
        return Ok(Some(format!(
            "Version '{rendered_version}' is larger than max_version '{max_version}'"
        )));
    }

    Ok(None)
}

fn normalized_version_requirement(raw: Option<&str>) -> Option<&str> {
    raw.map(str::trim).filter(|value| !value.is_empty())
}

fn compare_version_against_requirement(version: &Version, requirement: &str) -> Result<Ordering> {
    let expected = parse_version_requirement(requirement)?;
    let actual = [version.major, version.minor, version.patch];

    for index in 0..actual.len().min(expected.len()) {
        match actual[index].cmp(&expected[index]) {
            Ordering::Equal => continue,
            ordering => return Ok(ordering),
        }
    }

    Ok(Ordering::Equal)
}

fn parse_version_requirement(requirement: &str) -> Result<Vec<u16>> {
    requirement
        .split('.')
        .map(|segment| parse_version_segment(requirement, segment))
        .collect()
}

fn parse_version_segment(requirement: &str, segment: &str) -> Result<u16> {
    let digits: String = segment
        .chars()
        .take_while(|ch| ch.is_ascii_digit())
        .collect();
    if digits.is_empty() {
        return Err(runner_assertion_error(format!(
            "invalid version requirement '{requirement}': no numeric prefix in segment '{segment}'"
        )));
    }

    digits.parse::<u16>().map_err(|_| {
        runner_assertion_error(format!(
            "invalid version requirement '{requirement}': segment '{segment}' is out of range"
        ))
    })
}

fn format_version(version: &Version) -> String {
    format!("{}.{}.{}", version.major, version.minor, version.patch)
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

fn apply_statements(adapter: &mut dyn DatabaseAdapter, statements: &[Statement]) -> Result<()> {
    if statements.is_empty() {
        return Ok(());
    }

    let mut executor = Executor::new(adapter);
    executor.execute_plan(statements)
}

fn assert_online_idempotency(
    dialect: &dyn Dialect,
    adapter: &dyn DatabaseAdapter,
    expected_sql: &str,
    enable_drop: bool,
    phase: &str,
) -> Result<()> {
    let statements = generate_statements_from_export(dialect, adapter, expected_sql, enable_drop)?;
    if statements.is_empty() {
        return Ok(());
    }

    let rendered = Renderer::new(dialect).render(&statements);
    Err(runner_assertion_error(format!(
        "{phase} is not idempotent; expected no changes but got:\n{}",
        rendered.trim()
    )))
}

fn generate_statements_from_export(
    dialect: &dyn Dialect,
    adapter: &dyn DatabaseAdapter,
    desired_sql: &str,
    enable_drop: bool,
) -> Result<Vec<Statement>> {
    let current_sql = adapter.export_schema()?;
    generate_statements_with_search_path(
        dialect,
        desired_sql,
        &current_sql,
        enable_drop,
        adapter.schema_search_path(),
    )
}

fn generate_statements(
    dialect: &dyn Dialect,
    desired_sql: &str,
    current_sql: &str,
    enable_drop: bool,
) -> Result<Vec<Statement>> {
    generate_statements_with_search_path(dialect, desired_sql, current_sql, enable_drop, Vec::new())
}

fn generate_statements_with_search_path(
    dialect: &dyn Dialect,
    desired_sql: &str,
    current_sql: &str,
    enable_drop: bool,
    schema_search_path: Vec<String>,
) -> Result<Vec<Statement>> {
    let desired = parse_and_normalize(dialect, desired_sql)?;
    let current = parse_and_normalize(dialect, current_sql)?;
    let diff_config = DiffConfig::new(
        enable_drop,
        schema_search_path,
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
