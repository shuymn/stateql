mod adapter;
mod equivalence;
mod export_queries;
mod extra_keys;
mod generator;
mod normalize;
mod parser;
mod to_sql;

use stateql_core::{
    ConnectionConfig, DatabaseAdapter, Dialect, DiffOp, EquivalencePolicy, Ident, Result,
    SchemaObject, Statement,
};

#[derive(Debug, Default, Clone, Copy)]
pub struct MssqlDialect;

const DIALECT_NAME: &str = "mssql";

pub fn server_version_query() -> &'static str {
    export_queries::SHOW_SERVER_VERSION_QUERY
}

pub fn current_schema_query() -> &'static str {
    export_queries::CURRENT_SCHEMA_QUERY
}

pub fn table_names_query() -> &'static str {
    export_queries::TABLE_NAMES_QUERY
}

impl Dialect for MssqlDialect {
    fn name(&self) -> &str {
        DIALECT_NAME
    }

    fn parse(&self, sql: &str) -> Result<Vec<SchemaObject>> {
        parser::parse_schema(sql)
    }

    fn generate_ddl(&self, ops: &[DiffOp]) -> Result<Vec<Statement>> {
        generator::generate_ddl(self.name(), ops)
    }

    fn to_sql(&self, obj: &SchemaObject) -> Result<String> {
        to_sql::render_object(self.name(), obj)
    }

    fn normalize(&self, obj: &mut SchemaObject) {
        normalize::normalize_object(obj);
    }

    fn equivalence_policy(&self) -> &'static dyn EquivalencePolicy {
        &equivalence::MSSQL_EQUIVALENCE_POLICY
    }

    fn quote_ident(&self, ident: &Ident) -> String {
        format!("[{}]", ident.value)
    }

    fn batch_separator(&self) -> &str {
        "GO\n"
    }

    fn connect(&self, config: &ConnectionConfig) -> Result<Box<dyn DatabaseAdapter>> {
        adapter::connect(config)
    }
}
