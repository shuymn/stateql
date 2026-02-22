use std::collections::BTreeMap;

use super::{DataType, Expr, Ident, QualifiedName, Value};

#[derive(Debug, Clone, PartialEq)]
pub enum SchemaObject {
    Table(Table),
    View(View),
    MaterializedView(MaterializedView),
    Index(IndexDef),
    Sequence(Sequence),
    Trigger(Trigger),
    Function(Function),
    Type(TypeDef),
    Domain(Domain),
    Extension(Extension),
    Schema(SchemaDef),
    Comment(Comment),
    Privilege(Privilege),
    Policy(Policy),
}

#[derive(Debug, Clone, PartialEq)]
pub struct Table {
    pub name: QualifiedName,
    pub columns: Vec<Column>,
    pub primary_key: Option<PrimaryKey>,
    pub foreign_keys: Vec<ForeignKey>,
    pub checks: Vec<CheckConstraint>,
    pub exclusions: Vec<ExclusionConstraint>,
    pub options: TableOptions,
    pub partition: Option<Partition>,
    pub renamed_from: Option<Ident>,
}

impl Table {
    pub fn named(name: impl Into<String>) -> Self {
        Self {
            name: QualifiedName {
                schema: None,
                name: Ident::unquoted(name),
            },
            columns: Vec::new(),
            primary_key: None,
            foreign_keys: Vec::new(),
            checks: Vec::new(),
            exclusions: Vec::new(),
            options: TableOptions::default(),
            partition: None,
            renamed_from: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Column {
    pub name: Ident,
    pub data_type: DataType,
    pub not_null: bool,
    pub default: Option<Expr>,
    pub identity: Option<Identity>,
    pub generated: Option<GeneratedColumn>,
    pub comment: Option<String>,
    pub collation: Option<String>,
    pub renamed_from: Option<Ident>,
    pub extra: BTreeMap<String, Value>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Identity {
    pub always: bool,
    pub start: Option<i64>,
    pub increment: Option<i64>,
    pub min_value: Option<i64>,
    pub max_value: Option<i64>,
    pub cache: Option<i64>,
    pub cycle: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct GeneratedColumn {
    pub expr: Expr,
    pub stored: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PrimaryKey {
    pub name: Option<Ident>,
    pub columns: Vec<Ident>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ForeignKey {
    pub name: Option<Ident>,
    pub columns: Vec<Ident>,
    pub referenced_table: QualifiedName,
    pub referenced_columns: Vec<Ident>,
    pub on_delete: Option<ForeignKeyAction>,
    pub on_update: Option<ForeignKeyAction>,
    pub deferrable: Option<Deferrable>,
    pub extra: BTreeMap<String, Value>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ForeignKeyAction {
    NoAction,
    Restrict,
    Cascade,
    SetNull,
    SetDefault,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CheckConstraint {
    pub name: Option<Ident>,
    pub expr: Expr,
    pub no_inherit: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ExclusionConstraint {
    pub name: Option<Ident>,
    pub index_method: String,
    pub elements: Vec<ExclusionElement>,
    pub where_clause: Option<Expr>,
    pub deferrable: Option<Deferrable>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ExclusionElement {
    pub expr: Expr,
    pub operator: String,
    pub opclass: Option<String>,
    pub order: Option<SortOrder>,
    pub nulls: Option<NullsOrder>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Deferrable {
    Deferrable { initially_deferred: bool },
    NotDeferrable,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SortOrder {
    Asc,
    Desc,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NullsOrder {
    First,
    Last,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct TableOptions {
    pub extra: BTreeMap<String, Value>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct View {
    pub name: QualifiedName,
    pub columns: Vec<Ident>,
    pub query: String,
    pub check_option: Option<CheckOption>,
    pub security: Option<ViewSecurity>,
    pub renamed_from: Option<Ident>,
}

impl View {
    pub fn new(name: QualifiedName, query: impl Into<String>) -> Self {
        Self {
            name,
            columns: Vec::new(),
            query: query.into(),
            check_option: None,
            security: None,
            renamed_from: None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CheckOption {
    Local,
    Cascaded,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ViewSecurity {
    Definer,
    Invoker,
}

#[derive(Debug, Clone, PartialEq)]
pub struct MaterializedView {
    pub name: QualifiedName,
    pub columns: Vec<Column>,
    pub query: String,
    pub options: TableOptions,
    pub renamed_from: Option<Ident>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct IndexDef {
    pub name: Option<Ident>,
    pub owner: IndexOwner,
    pub columns: Vec<IndexColumn>,
    pub unique: bool,
    pub method: Option<String>,
    pub where_clause: Option<Expr>,
    pub concurrent: bool,
    pub extra: BTreeMap<String, Value>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct IndexColumn {
    pub expr: Expr,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IndexOwner {
    Table(QualifiedName),
    View(QualifiedName),
    MaterializedView(QualifiedName),
}

#[derive(Debug, Clone, PartialEq)]
pub struct Partition {
    pub strategy: PartitionStrategy,
    pub columns: Vec<Ident>,
    pub partitions: Vec<PartitionElement>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PartitionStrategy {
    Range,
    List,
    Hash,
    Key,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PartitionElement {
    pub name: Ident,
    pub bound: Option<PartitionBound>,
    pub extra: BTreeMap<String, Value>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum PartitionBound {
    LessThan(Vec<Expr>),
    In(Vec<Expr>),
    FromTo { from: Vec<Expr>, to: Vec<Expr> },
    MaxValue,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Sequence {
    pub name: QualifiedName,
    pub data_type: Option<DataType>,
    pub increment: Option<i64>,
    pub min_value: Option<i64>,
    pub max_value: Option<i64>,
    pub start: Option<i64>,
    pub cache: Option<i64>,
    pub cycle: bool,
    pub owned_by: Option<(QualifiedName, Ident)>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ColumnPosition {
    First,
    After(Ident),
}

#[derive(Debug, Clone, PartialEq)]
pub struct Trigger {
    pub name: QualifiedName,
    pub table: QualifiedName,
    pub timing: TriggerTiming,
    pub events: Vec<TriggerEvent>,
    pub for_each: TriggerForEach,
    pub when_clause: Option<Expr>,
    pub body: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TriggerTiming {
    Before,
    After,
    InsteadOf,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TriggerEvent {
    Insert,
    Update,
    Delete,
    Truncate,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TriggerForEach {
    Row,
    Statement,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Function {
    pub name: QualifiedName,
    pub params: Vec<FunctionParam>,
    pub return_type: Option<DataType>,
    pub language: String,
    pub body: String,
    pub volatility: Option<Volatility>,
    pub security: Option<FunctionSecurity>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct FunctionParam {
    pub name: Option<Ident>,
    pub data_type: DataType,
    pub mode: Option<FunctionParamMode>,
    pub default: Option<Expr>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FunctionParamMode {
    In,
    Out,
    InOut,
    Variadic,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Volatility {
    Immutable,
    Stable,
    Volatile,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FunctionSecurity {
    Definer,
    Invoker,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TypeDef {
    pub name: QualifiedName,
    pub kind: TypeKind,
}

#[derive(Debug, Clone, PartialEq)]
pub enum TypeKind {
    Enum { labels: Vec<String> },
    Composite { fields: Vec<(Ident, DataType)> },
    Range { subtype: DataType },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EnumValuePosition {
    Before(String),
    After(String),
}

#[derive(Debug, Clone, PartialEq)]
pub struct Domain {
    pub name: QualifiedName,
    pub data_type: DataType,
    pub default: Option<Expr>,
    pub not_null: bool,
    pub checks: Vec<CheckConstraint>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Extension {
    pub name: Ident,
    pub schema: Option<Ident>,
    pub version: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SchemaDef {
    pub name: Ident,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Comment {
    pub target: CommentTarget,
    pub text: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CommentTarget {
    Table(QualifiedName),
    Column { table: QualifiedName, column: Ident },
    Index(QualifiedName),
    View(QualifiedName),
    MaterializedView(QualifiedName),
    Sequence(QualifiedName),
    Trigger(QualifiedName),
    Function(QualifiedName),
    Type(QualifiedName),
    Domain(QualifiedName),
    Extension(Ident),
    Schema(Ident),
}

#[derive(Debug, Clone, PartialEq)]
pub struct Privilege {
    pub operations: Vec<PrivilegeOp>,
    pub on: PrivilegeObject,
    pub grantee: Ident,
    pub with_grant_option: bool,
}

impl Privilege {
    pub fn empty(on: PrivilegeObject, grantee: Ident) -> Self {
        Self {
            operations: Vec::new(),
            on,
            grantee,
            with_grant_option: false,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PrivilegeOp {
    Select,
    Insert,
    Update,
    Delete,
    Truncate,
    References,
    Trigger,
    Usage,
    Create,
    Connect,
    Temporary,
    Execute,
    All,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PrivilegeObject {
    Table(QualifiedName),
    View(QualifiedName),
    MaterializedView(QualifiedName),
    Sequence(QualifiedName),
    Schema(Ident),
    Database(Ident),
    Domain(QualifiedName),
    Type(QualifiedName),
    Function(QualifiedName),
}

#[derive(Debug, Clone, PartialEq)]
pub struct Policy {
    pub name: Ident,
    pub table: QualifiedName,
    pub command: Option<PolicyCommand>,
    pub using_expr: Option<Expr>,
    pub check_expr: Option<Expr>,
    pub roles: Vec<Ident>,
    pub permissive: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PolicyCommand {
    All,
    Select,
    Insert,
    Update,
    Delete,
}
