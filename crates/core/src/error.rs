use std::error::Error as StdError;

use crate::StatementContext;

type BoxedError = Box<dyn StdError + Send + Sync + 'static>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceLocation {
    pub line: usize,
    pub column: Option<usize>,
}

#[derive(Debug, thiserror::Error)]
pub enum ParseError {
    #[error(
        "parse statement[{statement_index}] failed: {source_sql} (source_location={})",
        format_location(.source_location.as_ref())
    )]
    StatementConversion {
        statement_index: usize,
        source_sql: String,
        source_location: Option<SourceLocation>,
        #[source]
        source: BoxedError,
    },
}

#[derive(Debug, thiserror::Error)]
pub enum DiffError {
    #[error("diff target `{target}` failed: {operation}")]
    ObjectComparison { target: String, operation: String },
}

#[derive(Debug, thiserror::Error)]
pub enum GenerateError {
    #[error("generate dialect `{dialect}` target `{target}` failed for op `{diff_op}`")]
    UnsupportedDiffOp {
        diff_op: String,
        target: String,
        dialect: String,
    },
}

#[derive(Debug, thiserror::Error)]
pub enum ExecutionError {
    #[error(
        "execute statement[{statement_index}] failed after {executed_statements} successes: {sql} (source_location={}, statement_context={})",
        format_location(.source_location.as_ref()),
        format_statement_context(.statement_context.as_deref())
    )]
    StatementFailed {
        statement_index: usize,
        sql: String,
        executed_statements: usize,
        source_location: Option<SourceLocation>,
        statement_context: Option<Box<StatementContext>>,
        #[source]
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

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("parse error: {0}")]
    Parse(#[from] ParseError),
    #[error("diff error: {0}")]
    Diff(#[from] DiffError),
    #[error("generate error: {0}")]
    Generate(#[from] GenerateError),
    #[error("execute error: {0}")]
    Execute(#[from] ExecutionError),
}

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
