use stateql_core::{
    ConnectionConfig, DatabaseAdapter, Dialect, DiffOp, GenerateError, Ident, Result, SchemaObject,
    Statement, Table, Transaction, Version,
};

#[derive(Debug, Default)]
struct ContractDialect;

#[derive(Debug, Default)]
struct DummyAdapter;

impl DatabaseAdapter for DummyAdapter {
    fn export_schema(&self) -> Result<String> {
        Ok("".to_string())
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
            major: 0,
            minor: 0,
            patch: 0,
        })
    }
}

impl Dialect for ContractDialect {
    fn name(&self) -> &str {
        "contract"
    }

    fn parse(&self, _sql: &str) -> Result<Vec<SchemaObject>> {
        Ok(vec![SchemaObject::Table(Table::named("users"))])
    }

    fn generate_ddl(&self, _ops: &[DiffOp]) -> Result<Vec<Statement>> {
        Ok(vec![Statement::Sql {
            sql: "CREATE TABLE users(id int);".to_string(),
            transactional: true,
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

    fn batch_separator(&self) -> &str {
        ""
    }

    fn connect(&self, _config: &ConnectionConfig) -> Result<Box<dyn DatabaseAdapter>> {
        Ok(Box::<DummyAdapter>::default())
    }
}

#[test]
fn dialect_trait_requires_full_contract_methods() {
    let dialect = ContractDialect;

    let parsed = dialect.parse("CREATE TABLE users(id int);").expect("parse");
    assert_eq!(parsed.len(), 1);

    let mut object = parsed[0].clone();
    dialect.normalize(&mut object);

    let ddl = dialect.generate_ddl(&[]).expect("generate_ddl");
    assert_eq!(ddl.len(), 1);

    let to_sql_error = dialect
        .to_sql(&object)
        .expect_err("to_sql should be stubbed");
    assert!(matches!(to_sql_error, stateql_core::Error::Generate(_)));

    let quoted = dialect.quote_ident(&Ident {
        value: "users".to_string(),
        quoted: false,
    });
    assert_eq!(quoted, "\"users\"");

    assert_eq!(dialect.batch_separator(), "");
    assert!(
        dialect
            .equivalence_policy()
            .is_equivalent_custom_type("int", "int")
    );

    let connection = ConnectionConfig {
        host: None,
        port: None,
        user: None,
        password: None,
        database: "db".to_string(),
        socket: None,
        extra: std::collections::BTreeMap::new(),
    };
    let adapter = dialect.connect(&connection).expect("connect");
    assert_eq!(adapter.export_schema().expect("export"), "");
}
