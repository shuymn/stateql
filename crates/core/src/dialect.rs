use crate::{
    ConnectionConfig, DatabaseAdapter, DiffOp, Ident, Result, SchemaObject, Statement,
    diff::{DEFAULT_EQUIVALENCE_POLICY, EquivalencePolicy},
};

pub trait Dialect: Send + Sync {
    fn name(&self) -> &str;
    fn parse(&self, sql: &str) -> Result<Vec<SchemaObject>>;
    fn generate_ddl(&self, ops: &[DiffOp]) -> Result<Vec<Statement>>;
    fn to_sql(&self, obj: &SchemaObject) -> Result<String>;
    fn normalize(&self, obj: &mut SchemaObject);
    fn equivalence_policy(&self) -> &'static dyn EquivalencePolicy {
        &DEFAULT_EQUIVALENCE_POLICY
    }
    fn quote_ident(&self, ident: &Ident) -> String;
    fn batch_separator(&self) -> &str {
        ""
    }
    fn connect(&self, config: &ConnectionConfig) -> Result<Box<dyn DatabaseAdapter>>;
}
