mod adapter;
mod dialect;
mod diff;
mod error;
mod ir;
mod statement;

pub use adapter::DatabaseAdapter;
pub use dialect::Dialect;
pub use diff::DiffOp;
pub use error::{CoreError, CoreResult};
pub use ir::{Ident, SchemaObject};
pub use statement::Statement;
