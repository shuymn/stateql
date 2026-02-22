use stateql_core::{Dialect, DiffOp, Result, SchemaObject, Statement};

#[derive(Debug, Default, Clone, Copy)]
pub struct SqliteDialect;

impl Dialect for SqliteDialect {
    fn name(&self) -> &'static str {
        "sqlite"
    }

    fn parse(&self, _sql: &str) -> Result<Vec<SchemaObject>> {
        Ok(Vec::new())
    }

    fn generate_ddl(&self, _ops: &[DiffOp]) -> Result<Vec<Statement>> {
        Ok(Vec::new())
    }
}
