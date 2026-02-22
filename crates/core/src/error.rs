#![allow(dead_code)]

use std::{error::Error as StdError, fmt};

type BoxedError = Box<dyn StdError + Send + Sync + 'static>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceLocation {
    pub line: usize,
    pub column: Option<usize>,
}

#[derive(Debug)]
#[allow(dead_code)]
pub enum ParseError {
    StatementConversion {
        statement_index: usize,
        source_sql: String,
        source_location: Option<SourceLocation>,
        source: BoxedError,
    },
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::StatementConversion {
                statement_index,
                source_sql,
                source_location,
                ..
            } => {
                write!(
                    f,
                    "parse statement[{statement_index}] failed: {source_sql} (source_location={})",
                    format_location(source_location.as_ref())
                )
            }
        }
    }
}

impl StdError for ParseError {
    fn source(&self) -> Option<&(dyn StdError + 'static)> {
        match self {
            Self::StatementConversion { source, .. } => Some(source.as_ref()),
        }
    }
}

#[derive(Debug)]
#[allow(dead_code)]
pub enum DiffError {
    ObjectComparison { target: String, operation: String },
}

impl fmt::Display for DiffError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ObjectComparison { target, operation } => {
                write!(f, "diff target `{target}` failed: {operation}")
            }
        }
    }
}

impl StdError for DiffError {}

#[derive(Debug)]
#[allow(dead_code)]
pub enum GenerateError {
    UnsupportedDiffOp {
        diff_op: String,
        target: String,
        dialect: String,
    },
}

impl fmt::Display for GenerateError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnsupportedDiffOp {
                diff_op,
                target,
                dialect,
            } => write!(
                f,
                "generate dialect `{dialect}` target `{target}` failed for op `{diff_op}`"
            ),
        }
    }
}

impl StdError for GenerateError {}

#[derive(Debug)]
#[allow(dead_code)]
pub enum ExecutionError {
    StatementFailed {
        statement_index: usize,
        sql: String,
        executed_statements: usize,
        source_location: Option<SourceLocation>,
        source: BoxedError,
    },
}

impl fmt::Display for ExecutionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::StatementFailed {
                statement_index,
                sql,
                executed_statements,
                source_location,
                ..
            } => write!(
                f,
                "execute statement[{statement_index}] failed after {executed_statements} successes: {sql} (source_location={})",
                format_location(source_location.as_ref())
            ),
        }
    }
}

impl StdError for ExecutionError {
    fn source(&self) -> Option<&(dyn StdError + 'static)> {
        match self {
            Self::StatementFailed { source, .. } => Some(source.as_ref()),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[allow(dead_code)]
pub struct CoreError {
    message: String,
}

impl CoreError {
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

impl fmt::Display for CoreError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl std::error::Error for CoreError {}

#[allow(dead_code)]
pub type CoreResult<T> = Result<T, CoreError>;

fn format_location(location: Option<&SourceLocation>) -> String {
    match location {
        Some(SourceLocation { line, column }) => match column {
            Some(column) => format!("{line}:{column}"),
            None => line.to_string(),
        },
        None => "unknown".to_string(),
    }
}
