use stateql_core::{CoreResult, Dialect, DiffOp, SchemaObject, Statement};

#[derive(Debug, Default, Clone, Copy)]
pub struct MysqlDialect;

impl Dialect for MysqlDialect {
    fn name(&self) -> &'static str {
        "mysql"
    }

    fn parse(&self, _sql: &str) -> CoreResult<Vec<SchemaObject>> {
        Ok(Vec::new())
    }

    fn generate_ddl(&self, _ops: &[DiffOp]) -> CoreResult<Vec<Statement>> {
        Ok(Vec::new())
    }
}
