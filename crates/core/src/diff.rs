use crate::SchemaObject;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DiffOp {
    CreateObject(SchemaObject),
    DropObject(SchemaObject),
}
