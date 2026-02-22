use std::error::Error as StdError;

use stateql_core::{
    ConnectionConfig, DatabaseAdapter, Dialect, DiffOp, ExecutionError, GenerateError, Ident,
    ParseError, Result, SchemaObject, SourceLocation, Statement,
};

#[derive(Debug, Default, Clone, Copy)]
pub struct MssqlDialect;

const DIALECT_NAME: &str = "mssql";
const DIALECT_TARGET: &str = "dialect contract";
const PARSE_NOT_IMPLEMENTED: &str = "mssql parse is not implemented";
const CONNECT_NOT_IMPLEMENTED: &str = "mssql connect is not implemented";
const GENERATE_DDL_STUB_OP: &str = "GenerateDdlStub";
const TO_SQL_STUB_OP: &str = "ToSqlStub";
const CONNECT_STUB_SQL: &str = "CONNECT mssql";

impl Dialect for MssqlDialect {
    fn name(&self) -> &str {
        DIALECT_NAME
    }

    fn parse(&self, sql: &str) -> Result<Vec<SchemaObject>> {
        Err(ParseError::StatementConversion {
            statement_index: 0,
            source_sql: sql.to_string(),
            source_location: Some(SourceLocation {
                line: 1,
                column: None,
            }),
            source: boxed_error(PARSE_NOT_IMPLEMENTED),
        }
        .into())
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
            source: boxed_error(CONNECT_NOT_IMPLEMENTED),
        }
        .into())
    }
}

fn boxed_error(message: &'static str) -> Box<dyn StdError + Send + Sync> {
    Box::new(std::io::Error::other(message))
}
