use crate::{DiffOp, Result, SchemaObject, Statement};

pub trait Dialect: Send + Sync {
    fn name(&self) -> &'static str;
    fn parse(&self, sql: &str) -> Result<Vec<SchemaObject>>;
    fn generate_ddl(&self, ops: &[DiffOp]) -> Result<Vec<Statement>>;
}
