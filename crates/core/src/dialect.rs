use crate::{CoreResult, DiffOp, SchemaObject, Statement};

pub trait Dialect: Send + Sync {
    fn name(&self) -> &'static str;
    fn parse(&self, sql: &str) -> CoreResult<Vec<SchemaObject>>;
    fn generate_ddl(&self, ops: &[DiffOp]) -> CoreResult<Vec<Statement>>;
}
