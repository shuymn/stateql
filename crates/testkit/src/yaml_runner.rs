use std::{
    cmp::Ordering,
    collections::{BTreeMap, BTreeSet},
    path::{Path, PathBuf},
    sync::Arc,
};

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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TestCaseFile {
    pub file_name: String,
    pub path: PathBuf,
    pub cases: BTreeMap<String, TestCase>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct IdempotencyManifest {
    pub dialects: BTreeMap<String, DialectIdempotencyManifest>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DialectIdempotencyManifest {
    pub entries: Vec<IdempotencyManifestEntry>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AssertionManifest {
    pub dialects: BTreeMap<String, DialectAssertionManifest>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DialectAssertionManifest {
    pub groups: BTreeMap<String, AssertionManifestGroup>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AssertionManifestGroup {
    pub entries: Vec<IdempotencyManifestEntry>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct IdempotencyManifestEntry {
    pub id: String,
    pub status: ManifestStatus,
    #[serde(default)]
    pub case: Option<String>,
    #[serde(default)]
    pub reason: Option<String>,
    #[serde(default)]
    pub tracking: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ManifestStatus {
    Ported,
    Skipped,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ManifestCoverage {
    pub ported: usize,
    pub skipped: usize,
    pub total: usize,
    pub coverage_rate: f64,
}

#[derive(Debug)]
enum RunnerOutcome {
    Executed(Result<()>),
    Skipped(String),
}

pub fn load_test_cases_from_str(yaml: &str) -> Result<BTreeMap<String, TestCase>> {
    serde_yaml::from_str(yaml).map_err(|source| parse_yaml_error(yaml, source))
}

pub fn load_test_cases_from_path(path: impl AsRef<Path>) -> Result<BTreeMap<String, TestCase>> {
    let path = path.as_ref();
    let yaml = std::fs::read_to_string(path).map_err(|source| parse_yaml_io_error(path, source))?;
    load_test_cases_from_str(&yaml)
}

pub fn load_test_cases_from_dir(path: impl AsRef<Path>) -> Result<Vec<TestCaseFile>> {
    let path = path.as_ref();
    let mut yaml_paths = Vec::new();
    let entries = std::fs::read_dir(path).map_err(|source| parse_yaml_io_error(path, source))?;
    for entry in entries {
        let entry = entry.map_err(|source| parse_yaml_io_error(path, source))?;
        let entry_path = entry.path();
        if !is_yaml_path(&entry_path) {
            continue;
        }
        yaml_paths.push(entry_path);
    }

    yaml_paths.sort();

    let mut files = Vec::with_capacity(yaml_paths.len());
    for file_path in yaml_paths {
        let cases = load_test_cases_from_path(&file_path)?;
        let file_name = file_path
            .file_name()
            .and_then(|name| name.to_str())
            .ok_or_else(|| {
                runner_assertion_error(format!(
                    "idempotency file path '{}' is not valid UTF-8",
                    file_path.display()
                ))
            })?
            .to_string();
        files.push(TestCaseFile {
            file_name,
            path: file_path,
            cases,
        });
    }

    Ok(files)
}

pub fn load_idempotency_manifest_from_path(path: impl AsRef<Path>) -> Result<IdempotencyManifest> {
    let path = path.as_ref();
    let yaml = std::fs::read_to_string(path).map_err(|source| parse_yaml_io_error(path, source))?;
    serde_yaml::from_str(&yaml).map_err(|source| parse_yaml_error(&yaml, source))
}

pub fn load_assertion_manifest_from_path(path: impl AsRef<Path>) -> Result<AssertionManifest> {
    let path = path.as_ref();
    let yaml = std::fs::read_to_string(path).map_err(|source| parse_yaml_io_error(path, source))?;
    serde_yaml::from_str(&yaml).map_err(|source| parse_yaml_error(&yaml, source))
}

pub fn validate_idempotency_manifest_entries(entries: &[IdempotencyManifestEntry]) -> Result<()> {
    validate_manifest_entries(entries)
}

pub fn validate_assertion_manifest_entries(entries: &[IdempotencyManifestEntry]) -> Result<()> {
    validate_manifest_entries(entries)
}

fn validate_manifest_entries(entries: &[IdempotencyManifestEntry]) -> Result<()> {
    let mut seen_ids: BTreeMap<String, ()> = BTreeMap::new();
    let mut seen_cases: BTreeMap<String, ()> = BTreeMap::new();

    for entry in entries {
        let id = normalized_manifest_text(Some(entry.id.as_str())).ok_or_else(|| {
            runner_assertion_error("manifest entry id must be a non-empty string")
        })?;
        if seen_ids.insert(id.to_string(), ()).is_some() {
            return Err(runner_assertion_error(format!(
                "manifest contains duplicate entry id '{id}'"
            )));
        }

        match entry.status {
            ManifestStatus::Ported => {
                let case = normalized_manifest_text(entry.case.as_deref()).ok_or_else(|| {
                    runner_assertion_error(format!(
                        "ported entry '{id}' must include a non-empty case reference"
                    ))
                })?;
                if seen_cases.insert(case.to_string(), ()).is_some() {
                    return Err(runner_assertion_error(format!(
                        "manifest contains duplicate ported case reference '{case}'"
                    )));
                }
            }
            ManifestStatus::Skipped => {
                if normalized_manifest_text(entry.reason.as_deref()).is_none() {
                    return Err(runner_assertion_error(format!(
                        "skipped entry '{id}' must include a non-empty reason"
                    )));
                }
                if normalized_manifest_text(entry.tracking.as_deref()).is_none() {
                    return Err(runner_assertion_error(format!(
                        "skipped entry '{id}' must include non-empty tracking"
                    )));
                }
            }
        }
    }

    Ok(())
}

pub fn idempotency_manifest_coverage(entries: &[IdempotencyManifestEntry]) -> ManifestCoverage {
    manifest_coverage(entries)
}

pub fn assertion_manifest_coverage(entries: &[IdempotencyManifestEntry]) -> ManifestCoverage {
    manifest_coverage(entries)
}

fn manifest_coverage(entries: &[IdempotencyManifestEntry]) -> ManifestCoverage {
    let mut ported = 0_usize;
    let mut skipped = 0_usize;

    for entry in entries {
        match entry.status {
            ManifestStatus::Ported => ported += 1,
            ManifestStatus::Skipped => skipped += 1,
        }
    }

    let total = ported + skipped;
    let coverage_rate = if total == 0 {
        0.0
    } else {
        ported as f64 / total as f64
    };

    ManifestCoverage {
        ported,
        skipped,
        total,
        coverage_rate,
    }
}

pub fn manifest_ported_case_references(
    entries: &[IdempotencyManifestEntry],
) -> Result<BTreeSet<String>> {
    let mut manifest_ported_cases = BTreeSet::new();
    for entry in entries {
        if entry.status != ManifestStatus::Ported {
            continue;
        }

        let case_ref = normalized_manifest_text(entry.case.as_deref()).ok_or_else(|| {
            runner_assertion_error(format!(
                "ported entry '{}' must include a non-empty case reference",
                entry.id
            ))
        })?;

        if !manifest_ported_cases.insert(case_ref.to_string()) {
            return Err(runner_assertion_error(format!(
                "manifest contains duplicate ported case reference '{case_ref}'"
            )));
        }
    }

    Ok(manifest_ported_cases)
}

pub fn yaml_case_references(files: &[TestCaseFile]) -> BTreeSet<String> {
    let mut cases = BTreeSet::new();
    for file in files {
        for case_name in file.cases.keys() {
            cases.insert(format!("{}::{}", file.file_name, case_name));
        }
    }
    cases
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

fn normalize_sql(sql: &str) -> String {
    sql.replace('\r', "")
        .split(';')
        .map(collapse_sql_whitespace)
        .filter(|statement| !statement.is_empty())
        .collect::<Vec<_>>()
        .join(";")
}

fn collapse_sql_whitespace(fragment: &str) -> String {
    fragment.split_whitespace().collect::<Vec<_>>().join(" ")
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

fn parse_yaml_io_error(path: &Path, source: std::io::Error) -> stateql_core::Error {
    ParseError::StatementConversion {
        statement_index: 0,
        source_sql: path.display().to_string(),
        source_location: None,
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

fn is_yaml_path(path: &Path) -> bool {
    if !path.is_file() {
        return false;
    }
    matches!(
        path.extension().and_then(|extension| extension.to_str()),
        Some("yml" | "yaml")
    )
}

fn normalized_manifest_text(value: Option<&str>) -> Option<&str> {
    value.map(str::trim).filter(|value| !value.is_empty())
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
