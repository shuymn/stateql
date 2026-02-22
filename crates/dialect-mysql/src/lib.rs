mod adapter;
mod equivalence;
mod export_queries;
mod extra_keys;
mod normalize;
mod parser;
mod to_sql;

use stateql_core::{
    ConnectionConfig, DatabaseAdapter, Dialect, DiffOp, EquivalencePolicy, GenerateError, Ident,
    Result, SchemaObject, Statement,
};

#[derive(Debug, Default, Clone, Copy)]
pub struct MysqlDialect;

const DIALECT_NAME: &str = "mysql";
const DIALECT_TARGET: &str = "dialect contract";
const GENERATE_DDL_STUB_OP: &str = "GenerateDdlStub";

pub fn table_names_query() -> &'static str {
    export_queries::TABLE_NAMES_QUERY
}

pub fn lower_case_table_names_query() -> &'static str {
    export_queries::LOWER_CASE_TABLE_NAMES_QUERY
}

impl Dialect for MysqlDialect {
    fn name(&self) -> &str {
        DIALECT_NAME
    }

    fn parse(&self, sql: &str) -> Result<Vec<SchemaObject>> {
        parser::parse_schema(sql)
    }

    fn generate_ddl(&self, _ops: &[DiffOp]) -> Result<Vec<Statement>> {
        Err(GenerateError::UnsupportedDiffOp {
            diff_op: GENERATE_DDL_STUB_OP.to_string(),
            target: DIALECT_TARGET.to_string(),
            dialect: self.name().to_string(),
        }
        .into())
    }

    fn to_sql(&self, obj: &SchemaObject) -> Result<String> {
        to_sql::render_object(self.name(), obj)
    }

    fn normalize(&self, obj: &mut SchemaObject) {
        normalize::normalize_object(obj);
    }

    fn equivalence_policy(&self) -> &'static dyn EquivalencePolicy {
        &equivalence::MYSQL_EQUIVALENCE_POLICY
    }

    fn quote_ident(&self, ident: &Ident) -> String {
        format!("`{}`", ident.value)
    }

    fn connect(&self, config: &ConnectionConfig) -> Result<Box<dyn DatabaseAdapter>> {
        adapter::connect(config)
    }
}
