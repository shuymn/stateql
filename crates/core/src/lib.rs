mod adapter;
mod config;
mod dialect;
mod diff;
mod error;
mod ir;
mod statement;

pub use adapter::{DatabaseAdapter, Transaction};
pub use config::{ConnectionConfig, Version};
pub use dialect::{
    DEFAULT_EQUIVALENCE_POLICY, DefaultEquivalencePolicy, Dialect, EquivalencePolicy,
};
pub use diff::DiffOp;
pub use error::{
    DiffError, Error, ExecutionError, GenerateError, ParseError, Result, SourceLocation,
};
pub use ir::{
    BinaryOperator, ComparisonOp, DataType, Expr, Ident, IsTest, Literal, QualifiedName,
    SchemaObject, SetQuantifier, SubQuery, UnaryOperator, Value, WindowSpec, float_total_cmp,
    value_total_eq,
};
pub use statement::Statement;
