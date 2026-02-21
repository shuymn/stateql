use stateql_core::{CoreResult, Dialect, DiffOp, SchemaObject, Statement};

#[derive(Debug, Default, Clone, Copy)]
pub struct PostgresDialect;

impl Dialect for PostgresDialect {
    fn name(&self) -> &'static str {
        "postgres"
    }

    fn parse(&self, _sql: &str) -> CoreResult<Vec<SchemaObject>> {
        Ok(Vec::new())
    }

    fn generate_ddl(&self, _ops: &[DiffOp]) -> CoreResult<Vec<Statement>> {
        Ok(Vec::new())
    }
}
