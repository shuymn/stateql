use std::collections::BTreeMap;

use stateql_core::{
    ConnectionConfig, DatabaseAdapter, Dialect, DiffOp, GenerateError, Ident, Result, SchemaObject,
    Statement, Table, Transaction, Version,
};

#[derive(Debug, Default)]
struct TemplateDialect;

#[derive(Debug, Default)]
struct TemplateAdapter;

impl DatabaseAdapter for TemplateAdapter {
    fn export_schema(&self) -> Result<String> {
        Ok(String::new())
    }

    fn execute(&self, _sql: &str) -> Result<()> {
        Ok(())
    }

    fn begin(&mut self) -> Result<Transaction<'_>> {
        Ok(Transaction::new(self))
    }

    fn schema_search_path(&self) -> Vec<String> {
        vec!["public".to_string()]
    }

    fn server_version(&self) -> Result<Version> {
        Ok(Version {
            major: 13,
            minor: 0,
            patch: 0,
        })
    }
}

impl Dialect for TemplateDialect {
    fn name(&self) -> &str {
        "template"
    }

    fn parse(&self, _sql: &str) -> Result<Vec<SchemaObject>> {
        Ok(vec![SchemaObject::Table(Table::named("users"))])
    }

    fn generate_ddl(&self, _ops: &[DiffOp]) -> Result<Vec<Statement>> {
        Ok(vec![Statement::Sql {
            sql: "CREATE TABLE users(id int);".to_string(),
            transactional: true,
            context: None,
        }])
    }

    fn to_sql(&self, _obj: &SchemaObject) -> Result<String> {
        Err(GenerateError::UnsupportedDiffOp {
            diff_op: "ToSql".to_string(),
            target: "users".to_string(),
            dialect: self.name().to_string(),
        }
        .into())
    }

    fn normalize(&self, _obj: &mut SchemaObject) {}

    fn quote_ident(&self, ident: &Ident) -> String {
        format!("\"{}\"", ident.value)
    }

    fn connect(&self, _config: &ConnectionConfig) -> Result<Box<dyn DatabaseAdapter>> {
        Ok(Box::<TemplateAdapter>::default())
    }
}

fn main() -> Result<()> {
    let dialect = TemplateDialect;
    let connection = ConnectionConfig {
        host: None,
        port: None,
        user: None,
        password: None,
        database: "db".to_string(),
        socket: None,
        extra: BTreeMap::new(),
    };

    let mut objects = dialect.parse("CREATE TABLE users(id int);")?;
    dialect.normalize(&mut objects[0]);
    let statements = dialect.generate_ddl(&[])?;
    let adapter = dialect.connect(&connection)?;

    assert_eq!(dialect.name(), "template");
    assert_eq!(statements.len(), 1);
    assert_eq!(adapter.export_schema()?, "");

    let unsupported = dialect.to_sql(&objects[0]).expect_err("unsupported");
    assert!(matches!(
        unsupported,
        stateql_core::Error::Generate(GenerateError::UnsupportedDiffOp { .. })
    ));

    Ok(())
}
