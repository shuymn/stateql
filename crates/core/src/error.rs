#![allow(dead_code)]

use std::{error::Error as StdError, fmt};

use crate::StatementContext;

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
        statement_context: Option<Box<StatementContext>>,
        source: BoxedError,
    },
}

impl ExecutionError {
    pub fn statement_failed<E>(
        statement_index: usize,
        sql: impl Into<String>,
        executed_statements: usize,
        source_location: Option<SourceLocation>,
        statement_context: Option<StatementContext>,
        source: E,
    ) -> Self
    where
        E: StdError + Send + Sync + 'static,
    {
        Self::StatementFailed {
            statement_index,
            sql: sql.into(),
            executed_statements,
            source_location,
            statement_context: statement_context.map(Box::new),
            source: Box::new(source),
        }
    }
}

impl fmt::Display for ExecutionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::StatementFailed {
                statement_index,
                sql,
                executed_statements,
                source_location,
                statement_context,
                ..
            } => write!(
                f,
                "execute statement[{statement_index}] failed after {executed_statements} successes: {sql} (source_location={}, statement_context={})",
                format_location(source_location.as_ref()),
                format_statement_context(statement_context.as_deref())
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

#[derive(Debug)]
#[allow(dead_code)]
pub enum Error {
    Parse(ParseError),
    Diff(DiffError),
    Generate(GenerateError),
    Execute(ExecutionError),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Parse(error) => write!(f, "parse error: {error}"),
            Self::Diff(error) => write!(f, "diff error: {error}"),
            Self::Generate(error) => write!(f, "generate error: {error}"),
            Self::Execute(error) => write!(f, "execute error: {error}"),
        }
    }
}

impl StdError for Error {
    fn source(&self) -> Option<&(dyn StdError + 'static)> {
        match self {
            Self::Parse(error) => Some(error),
            Self::Diff(error) => Some(error),
            Self::Generate(error) => Some(error),
            Self::Execute(error) => Some(error),
        }
    }
}

impl From<ParseError> for Error {
    fn from(value: ParseError) -> Self {
        Self::Parse(value)
    }
}

impl From<DiffError> for Error {
    fn from(value: DiffError) -> Self {
        Self::Diff(value)
    }
}

impl From<GenerateError> for Error {
    fn from(value: GenerateError) -> Self {
        Self::Generate(value)
    }
}

impl From<ExecutionError> for Error {
    fn from(value: ExecutionError) -> Self {
        Self::Execute(value)
    }
}

#[allow(dead_code)]
pub type Result<T> = std::result::Result<T, Error>;

fn format_location(location: Option<&SourceLocation>) -> String {
    match location {
        Some(SourceLocation { line, column }) => match column {
            Some(column) => format!("{line}:{column}"),
            None => line.to_string(),
        },
        None => "unknown".to_string(),
    }
}

fn format_statement_context(statement_context: Option<&StatementContext>) -> String {
    match statement_context {
        Some(context) => format!("{context:?}"),
        None => "none".to_string(),
    }
}
