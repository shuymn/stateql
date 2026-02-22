use std::{
    error::Error as StdError,
    fmt,
    sync::{Mutex, MutexGuard},
};

use stateql_core::{
    ConnectionConfig, DatabaseAdapter, Dialect, DiffOp, GenerateError, Ident, ParseError, Result,
    SchemaObject, Statement, Table,
};

#[derive(Debug, Default)]
pub struct OfflineFakeDialect {
    state: Mutex<State>,
}

#[derive(Debug, Default)]
struct State {
    generated_batches: Vec<Vec<DiffOp>>,
}

impl OfflineFakeDialect {
    pub fn generated_batches(&self) -> Vec<Vec<DiffOp>> {
        self.state_guard().generated_batches.clone()
    }

    pub fn expected_error_message(message: &str) -> String {
        format!("parse error: parse statement[0] failed: {message} (source_location=unknown)")
    }

    fn state_guard(&self) -> MutexGuard<'_, State> {
        self.state
            .lock()
            .expect("offline fake dialect mutex should lock")
    }
}

impl Dialect for OfflineFakeDialect {
    fn name(&self) -> &str {
        "offline_fake"
    }

    fn parse(&self, sql: &str) -> Result<Vec<SchemaObject>> {
        parse_fake_schema(sql)
    }

    fn generate_ddl(&self, ops: &[DiffOp]) -> Result<Vec<Statement>> {
        self.state_guard().generated_batches.push(ops.to_vec());

        ops.iter().map(diff_op_to_statement).collect()
    }

    fn to_sql(&self, obj: &SchemaObject) -> Result<String> {
        match obj {
            SchemaObject::Table(table) => Ok(format!("CREATE TABLE {};", table.name.name.value)),
            _ => Err(unsupported_op_error("to_sql", "unsupported schema object")),
        }
    }

    fn normalize(&self, _obj: &mut SchemaObject) {}

    fn quote_ident(&self, ident: &Ident) -> String {
        ident.value.clone()
    }

    fn connect(&self, _config: &ConnectionConfig) -> Result<Box<dyn DatabaseAdapter>> {
        Err(unsupported_op_error(
            "connect",
            "adapter unavailable in offline fake dialect",
        ))
    }
}

fn parse_fake_schema(sql: &str) -> Result<Vec<SchemaObject>> {
    let trimmed = sql.trim();
    if trimmed.is_empty() {
        return Ok(Vec::new());
    }

    if let Some(message) = trimmed.strip_prefix("ERROR:") {
        return Err(parse_error(message.trim(), trimmed));
    }

    let Some(tables) = trimmed.strip_prefix("tables:") else {
        return Err(parse_error(
            "expected fake schema format `tables:<name[,name...]>`",
            trimmed,
        ));
    };

    let objects = tables
        .split(',')
        .map(str::trim)
        .filter(|name| !name.is_empty())
        .map(|name| SchemaObject::Table(Table::named(name)))
        .collect();

    Ok(objects)
}

fn diff_op_to_statement(op: &DiffOp) -> Result<Statement> {
    let sql = match op {
        DiffOp::CreateTable(table) => format!("CREATE TABLE {};", table.name.name.value),
        DiffOp::DropTable(name) => format!("DROP TABLE {};", name.name.value),
        _ => {
            return Err(unsupported_op_error(
                diff_op_name(op),
                "unsupported fake diff operation",
            ));
        }
    };

    Ok(Statement::Sql {
        sql,
        transactional: true,
        context: None,
    })
}

fn diff_op_name(op: &DiffOp) -> &'static str {
    match op {
        DiffOp::CreateTable(_) => "CreateTable",
        DiffOp::DropTable(_) => "DropTable",
        DiffOp::RenameTable { .. } => "RenameTable",
        DiffOp::AddColumn { .. } => "AddColumn",
        DiffOp::DropColumn { .. } => "DropColumn",
        DiffOp::AlterColumn { .. } => "AlterColumn",
        DiffOp::RenameColumn { .. } => "RenameColumn",
        DiffOp::AddIndex(_) => "AddIndex",
        DiffOp::DropIndex { .. } => "DropIndex",
        DiffOp::RenameIndex { .. } => "RenameIndex",
        DiffOp::AddForeignKey { .. } => "AddForeignKey",
        DiffOp::DropForeignKey { .. } => "DropForeignKey",
        DiffOp::AddCheck { .. } => "AddCheck",
        DiffOp::DropCheck { .. } => "DropCheck",
        DiffOp::AddExclusion { .. } => "AddExclusion",
        DiffOp::DropExclusion { .. } => "DropExclusion",
        DiffOp::SetPrimaryKey { .. } => "SetPrimaryKey",
        DiffOp::DropPrimaryKey { .. } => "DropPrimaryKey",
        DiffOp::AddPartition { .. } => "AddPartition",
        DiffOp::DropPartition { .. } => "DropPartition",
        DiffOp::CreateView(_) => "CreateView",
        DiffOp::DropView(_) => "DropView",
        DiffOp::CreateMaterializedView(_) => "CreateMaterializedView",
        DiffOp::DropMaterializedView(_) => "DropMaterializedView",
        DiffOp::CreateSequence(_) => "CreateSequence",
        DiffOp::DropSequence(_) => "DropSequence",
        DiffOp::AlterSequence { .. } => "AlterSequence",
        DiffOp::CreateTrigger(_) => "CreateTrigger",
        DiffOp::DropTrigger { .. } => "DropTrigger",
        DiffOp::CreateFunction(_) => "CreateFunction",
        DiffOp::DropFunction(_) => "DropFunction",
        DiffOp::CreateType(_) => "CreateType",
        DiffOp::DropType(_) => "DropType",
        DiffOp::AlterType { .. } => "AlterType",
        DiffOp::CreateDomain(_) => "CreateDomain",
        DiffOp::DropDomain(_) => "DropDomain",
        DiffOp::AlterDomain { .. } => "AlterDomain",
        DiffOp::CreateExtension(_) => "CreateExtension",
        DiffOp::DropExtension(_) => "DropExtension",
        DiffOp::CreateSchema(_) => "CreateSchema",
        DiffOp::DropSchema(_) => "DropSchema",
        DiffOp::SetComment(_) => "SetComment",
        DiffOp::DropComment { .. } => "DropComment",
        DiffOp::Grant(_) => "Grant",
        DiffOp::Revoke(_) => "Revoke",
        DiffOp::CreatePolicy(_) => "CreatePolicy",
        DiffOp::DropPolicy { .. } => "DropPolicy",
        DiffOp::AlterTableOptions { .. } => "AlterTableOptions",
    }
}

fn parse_error(message: &str, source_sql: &str) -> stateql_core::Error {
    ParseError::StatementConversion {
        statement_index: 0,
        source_sql: message.to_string(),
        source_location: None,
        source: Box::new(FakeSourceError(source_sql.to_string())),
    }
    .into()
}

fn unsupported_op_error(diff_op: &str, target: &str) -> stateql_core::Error {
    GenerateError::UnsupportedDiffOp {
        diff_op: diff_op.to_string(),
        target: target.to_string(),
        dialect: "offline_fake".to_string(),
    }
    .into()
}

#[derive(Debug)]
struct FakeSourceError(String);

impl fmt::Display for FakeSourceError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl StdError for FakeSourceError {}
