use std::error::Error as StdError;

mod extra_keys;
mod parser;

use stateql_core::{
    ConnectionConfig, DatabaseAdapter, Dialect, DiffOp, ExecutionError, GenerateError, Ident,
    Result, SchemaObject, Statement,
};

#[derive(Debug, Default, Clone, Copy)]
pub struct MssqlDialect;

const DIALECT_NAME: &str = "mssql";
const DIALECT_TARGET: &str = "dialect contract";
const CONNECT_NOT_IMPLEMENTED: &str = "mssql connect is not implemented";
const GENERATE_DDL_STUB_OP: &str = "GenerateDdlStub";
const TO_SQL_STUB_OP: &str = "ToSqlStub";
const CONNECT_STUB_SQL: &str = "CONNECT mssql";

impl Dialect for MssqlDialect {
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

    fn to_sql(&self, _obj: &SchemaObject) -> Result<String> {
        Err(GenerateError::UnsupportedDiffOp {
            diff_op: TO_SQL_STUB_OP.to_string(),
            target: DIALECT_TARGET.to_string(),
            dialect: self.name().to_string(),
        }
        .into())
    }

    fn normalize(&self, _obj: &mut SchemaObject) {}

    fn quote_ident(&self, ident: &Ident) -> String {
        format!("[{}]", ident.value)
    }

    fn batch_separator(&self) -> &str {
        "GO\n"
    }

    fn connect(&self, _config: &ConnectionConfig) -> Result<Box<dyn DatabaseAdapter>> {
        Err(ExecutionError::StatementFailed {
            statement_index: 0,
            sql: CONNECT_STUB_SQL.to_string(),
            executed_statements: 0,
            source_location: None,
            statement_context: None,
            source: boxed_error(CONNECT_NOT_IMPLEMENTED),
        }
        .into())
    }
}

fn boxed_error(message: &'static str) -> Box<dyn StdError + Send + Sync> {
    Box::new(std::io::Error::other(message))
}
