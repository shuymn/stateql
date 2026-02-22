use std::error::Error as StdError;

use stateql_core::{
    DiffError, Error, ExecutionError, GenerateError, ParseError, Result, SourceLocation,
};

#[test]
fn top_level_error_wraps_stage_errors_with_from() {
    let parse = ParseError::StatementConversion {
        statement_index: 2,
        source_sql: "CREATE TABL broken;".to_string(),
        source_location: Some(SourceLocation {
            line: 9,
            column: Some(3),
        }),
        source: boxed_error("parse failed"),
    };

    let diff = DiffError::ObjectComparison {
        target: "users".to_string(),
        operation: "rename mismatch".to_string(),
    };

    let generate = GenerateError::UnsupportedDiffOp {
        diff_op: "AddExclusion".to_string(),
        target: "users".to_string(),
        dialect: "sqlite".to_string(),
    };

    let execute = ExecutionError::StatementFailed {
        statement_index: 4,
        sql: "ALTER TABLE users DROP COLUMN x;".to_string(),
        executed_statements: 2,
        source_location: None,
        source: boxed_error("execute failed"),
    };

    let wrapped_parse: Error = parse.into();
    let wrapped_diff: Error = diff.into();
    let wrapped_generate: Error = generate.into();
    let wrapped_execute: Error = execute.into();

    assert!(matches!(wrapped_parse, Error::Parse(_)));
    assert!(matches!(wrapped_diff, Error::Diff(_)));
    assert!(matches!(wrapped_generate, Error::Generate(_)));
    assert!(matches!(wrapped_execute, Error::Execute(_)));
}

#[test]
fn result_alias_uses_top_level_error() {
    fn fail() -> Result<()> {
        Err(DiffError::ObjectComparison {
            target: "orders".to_string(),
            operation: "missing owner".to_string(),
        }
        .into())
    }

    let err = fail().expect_err("must return top-level error");
    assert!(matches!(err, Error::Diff(_)));
}

fn boxed_error(message: &'static str) -> Box<dyn StdError + Send + Sync> {
    Box::new(std::io::Error::other(message))
}
