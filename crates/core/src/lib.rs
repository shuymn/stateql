mod adapter;
mod dialect;
mod diff;
mod error;
mod ir;
mod statement;

pub use adapter::DatabaseAdapter;
pub use dialect::Dialect;
pub use diff::DiffOp;
pub use error::{
    DiffError, Error, ExecutionError, GenerateError, ParseError, Result, SourceLocation,
};
pub use ir::{Ident, SchemaObject};
pub use statement::Statement;
