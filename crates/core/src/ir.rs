mod expr;
mod ident;
mod types;

pub use expr::{
    BinaryOperator, ComparisonOp, Expr, IsTest, Literal, SetQuantifier, SubQuery, UnaryOperator,
    WindowSpec,
};
pub use ident::{Ident, QualifiedName};
pub use types::{DataType, Value, float_total_cmp, value_total_eq};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SchemaObject {
    Table { name: String },
    Index { name: String, table: String },
}
