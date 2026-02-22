use std::error::Error as StdError;

use stateql_core::{DiffError, ExecutionError, GenerateError, ParseError, SourceLocation};

#[test]
fn stage_typed_errors_and_source_location_exist() {
    let location = SourceLocation {
        line: 42,
        column: Some(7),
    };

    let parse = ParseError::StatementConversion {
        statement_index: 1,
        source_sql: "CREATE TABL users (id int);".to_string(),
        source_location: Some(location.clone()),
        source: boxed_error("parse conversion failed"),
    };

    let diff = DiffError::ObjectComparison {
        target: "users".to_string(),
        operation: "rename annotation mismatch".to_string(),
    };

    let generate = GenerateError::UnsupportedDiffOp {
        diff_op: "AddExclusion".to_string(),
        target: "users".to_string(),
        dialect: "sqlite".to_string(),
    };

    let execute = ExecutionError::StatementFailed {
        statement_index: 3,
        sql: "ALTER TABLE users DROP COLUMN legacy;".to_string(),
        executed_statements: 2,
        source_location: Some(location),
        statement_context: None,
        source: boxed_error("execution failed"),
    };

    assert!(format!("{parse}").contains("statement[1]"));
    assert!(format!("{diff}").contains("users"));
    assert!(format!("{generate}").contains("sqlite"));
    assert!(format!("{execute}").contains("statement[3]"));
    assert!(format!("{execute}").contains("statement_context=none"));
}

#[test]
fn parse_error_statement_conversion_keeps_statement_context_and_location() {
    let location = SourceLocation {
        line: 12,
        column: Some(4),
    };

    let error = ParseError::StatementConversion {
        statement_index: 9,
        source_sql: "CREATE TABL broken;".to_string(),
        source_location: Some(location.clone()),
        source: boxed_error("unexpected token"),
    };

    match error {
        ParseError::StatementConversion {
            statement_index,
            source_sql,
            source_location,
            ..
        } => {
            assert_eq!(statement_index, 9);
            assert_eq!(source_sql, "CREATE TABL broken;");
            assert_eq!(source_location, Some(location));
        }
    }
}

fn boxed_error(message: &'static str) -> Box<dyn StdError + Send + Sync> {
    Box::new(std::io::Error::other(message))
}
