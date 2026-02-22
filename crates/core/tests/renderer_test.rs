use stateql_core::{
    ConnectionConfig, DatabaseAdapter, Dialect, DiffOp, Ident, Result, SchemaObject, Statement,
    Table, Transaction, Version,
};

#[derive(Debug, Default)]
struct RendererDialect;

#[derive(Debug, Default)]
struct DummyAdapter;

impl DatabaseAdapter for DummyAdapter {
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
        vec!["dbo".to_string()]
    }

    fn server_version(&self) -> Result<Version> {
        Ok(Version {
            major: 0,
            minor: 0,
            patch: 0,
        })
    }
}

impl Dialect for RendererDialect {
    fn name(&self) -> &str {
        "mssql-like"
    }

    fn parse(&self, _sql: &str) -> Result<Vec<SchemaObject>> {
        Ok(vec![SchemaObject::Table(Table::named("users"))])
    }

    fn generate_ddl(&self, _ops: &[DiffOp]) -> Result<Vec<Statement>> {
        Ok(vec![])
    }

    fn to_sql(&self, _obj: &SchemaObject) -> Result<String> {
        Ok(String::new())
    }

    fn normalize(&self, _obj: &mut SchemaObject) {}

    fn quote_ident(&self, ident: &Ident) -> String {
        format!("[{}]", ident.value)
    }

    fn batch_separator(&self) -> &str {
        "GO"
    }

    fn connect(&self, _config: &ConnectionConfig) -> Result<Box<dyn DatabaseAdapter>> {
        Ok(Box::<DummyAdapter>::default())
    }
}

#[test]
fn renderer_emits_batch_separator_for_batch_boundary() {
    let dialect = RendererDialect;
    let renderer = stateql_core::Renderer::new(&dialect);
    let statements = vec![
        Statement::Sql {
            sql: "CREATE TABLE users(id int);".to_string(),
            transactional: true,
            context: None,
        },
        Statement::BatchBoundary,
        Statement::Sql {
            sql: "ALTER TABLE users ADD name nvarchar(255);".to_string(),
            transactional: true,
            context: None,
        },
    ];

    let rendered = renderer.render(&statements);

    assert_eq!(
        rendered,
        "CREATE TABLE users(id int);\nGO\nALTER TABLE users ADD name nvarchar(255);\n"
    );
}
