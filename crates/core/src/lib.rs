mod adapter;
mod annotation;
mod config;
mod dialect;
mod diff;
mod error;
mod ir;
mod statement;

pub use adapter::{DatabaseAdapter, Transaction};
pub use annotation::{
    AnnotationAttachment, AnnotationExtractor, AnnotationTarget, RenameAnnotation,
    attach_annotations,
};
pub use config::{ConnectionConfig, Version};
pub use dialect::Dialect;
pub use diff::{
    ColumnChange, DEFAULT_EQUIVALENCE_POLICY, DefaultEquivalencePolicy, DiffConfig, DiffEngine,
    DiffOp, DomainChange, EquivalencePolicy, EquivalencePolicyContractError, SequenceChange,
    TypeChange, custom_types_equivalent, exprs_equivalent,
    is_mysql_change_column_full_redefinition, verify_equivalence_policy_contract,
};
pub use error::{
    DiffError, Error, ExecutionError, GenerateError, ParseError, Result, SourceLocation,
};
pub use ir::{
    BinaryOperator, CheckConstraint, CheckOption, Column, ColumnPosition, Comment, CommentTarget,
    ComparisonOp, DataType, Deferrable, Domain, EnumValuePosition, ExclusionConstraint,
    ExclusionElement, Expr, Extension, ForeignKey, ForeignKeyAction, Function, FunctionParam,
    FunctionParamMode, FunctionSecurity, GeneratedColumn, Ident, Identity, IndexColumn, IndexDef,
    IndexOwner, IsTest, Literal, MaterializedView, NullsOrder, Partition, PartitionBound,
    PartitionElement, PartitionStrategy, Policy, PolicyCommand, PrimaryKey, Privilege,
    PrivilegeObject, PrivilegeOp, QualifiedName, SchemaDef, SchemaObject, Sequence, SetQuantifier,
    SortOrder, SubQuery, Table, TableOptions, Trigger, TriggerEvent, TriggerForEach, TriggerTiming,
    TypeDef, TypeKind, UnaryOperator, Value, View, ViewSecurity, Volatility, WindowSpec,
    extra_keys, float_total_cmp, value_total_eq,
};
pub use statement::{SqliteRebuildStep, Statement, StatementContext};
