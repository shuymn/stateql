use std::collections::BTreeMap;

use super::{DataType, Expr, Ident, QualifiedName, Value};

#[derive(Debug, Clone, PartialEq)]
pub enum SchemaObject {
    Table(Table),
    View(View),
    MaterializedView(MaterializedView),
    Index(IndexDef),
    Sequence(Sequence),
}

#[derive(Debug, Clone, PartialEq)]
pub struct Table {
    pub name: QualifiedName,
    pub columns: Vec<Column>,
    pub primary_key: Option<PrimaryKey>,
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

#[derive(Debug, Clone, PartialEq, Default)]
pub struct TableOptions {
    pub extra: BTreeMap<String, Value>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct View {
    pub name: QualifiedName,
    pub columns: Vec<Ident>,
    pub query: String,
    pub renamed_from: Option<Ident>,
}

impl View {
    pub fn new(name: QualifiedName, query: impl Into<String>) -> Self {
        Self {
            name,
            columns: Vec::new(),
            query: query.into(),
            renamed_from: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct MaterializedView {
    pub name: QualifiedName,
    pub columns: Vec<Ident>,
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
