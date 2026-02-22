use stateql_core::{Dialect, DiffOp, Result, SchemaObject, Statement};

#[derive(Debug, Default, Clone, Copy)]
pub struct MssqlDialect;

impl Dialect for MssqlDialect {
    fn name(&self) -> &'static str {
        "mssql"
    }

    fn parse(&self, _sql: &str) -> Result<Vec<SchemaObject>> {
        Ok(Vec::new())
    }

    fn generate_ddl(&self, _ops: &[DiffOp]) -> Result<Vec<Statement>> {
        Ok(Vec::new())
    }
}
