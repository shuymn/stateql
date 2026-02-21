#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SchemaObject {
    Table { name: String },
    Index { name: String, table: String },
}
