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
    BinaryOperator, Column, ColumnPosition, ComparisonOp, DataType, Expr, GeneratedColumn, Ident,
    Identity, IndexColumn, IndexDef, IndexOwner, IsTest, Literal, MaterializedView, Partition,
    PartitionBound, PartitionElement, PartitionStrategy, PrimaryKey, QualifiedName, SchemaObject,
    Sequence, SetQuantifier, SubQuery, Table, TableOptions, UnaryOperator, Value, View, WindowSpec,
    float_total_cmp, value_total_eq,
};
pub use statement::Statement;
