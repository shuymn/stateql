use std::collections::BTreeMap;

use serde::Deserialize;
use stateql_core::{ParseError, Result, SourceLocation};

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

pub fn load_test_cases_from_str(yaml: &str) -> Result<BTreeMap<String, TestCase>> {
    serde_yaml::from_str(yaml).map_err(|source| parse_yaml_error(yaml, source))
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
