use std::{
    collections::BTreeMap,
    error::Error as StdError,
    fmt,
    sync::{Arc, Mutex},
};

use stateql_core::{
    ConnectionConfig, DEFAULT_EQUIVALENCE_POLICY, DatabaseAdapter, Dialect, DiffOp,
    EquivalencePolicy, Ident, ParseError, Result, SchemaObject, Statement, Transaction, Version,
};

#[derive(Debug, Clone)]
struct FakeAdapter {
    state: Arc<Mutex<FakeAdapterState>>,
}

#[derive(Debug)]
struct FakeAdapterState {
    export_schema_sql: String,
    schema_search_path: Vec<String>,
    executed_sql: Vec<String>,
}

impl Default for FakeAdapterState {
    fn default() -> Self {
        Self {
            export_schema_sql: String::new(),
            schema_search_path: vec!["public".to_string()],
            executed_sql: Vec::new(),
        }
    }
}

impl FakeAdapter {
    fn new(state: Arc<Mutex<FakeAdapterState>>) -> Self {
        Self { state }
    }
}

impl DatabaseAdapter for FakeAdapter {
    fn export_schema(&self) -> Result<String> {
        Ok(self
            .state
            .lock()
            .expect("fake adapter mutex should lock")
            .export_schema_sql
            .clone())
    }

    fn execute(&self, sql: &str) -> Result<()> {
        self.state
            .lock()
            .expect("fake adapter mutex should lock")
            .executed_sql
            .push(sql.to_string());
        Ok(())
    }

    fn begin(&mut self) -> Result<Transaction<'_>> {
        self.execute("BEGIN")?;
        Ok(Transaction::new(self))
    }

    fn schema_search_path(&self) -> Vec<String> {
        self.state
            .lock()
            .expect("fake adapter mutex should lock")
            .schema_search_path
            .clone()
    }

    fn server_version(&self) -> Result<Version> {
        Ok(Version {
            major: 0,
            minor: 0,
            patch: 0,
        })
    }
}

pub struct FakeDialect {
    state: Arc<Mutex<FakeDialectState>>,
    policy: &'static dyn EquivalencePolicy,
}

#[derive(Debug)]
struct FakeDialectState {
    parse_results: BTreeMap<String, Vec<SchemaObject>>,
    generated_ops: Vec<DiffOp>,
    adapter_state: Arc<Mutex<FakeAdapterState>>,
}

impl Default for FakeDialectState {
    fn default() -> Self {
        Self {
            parse_results: BTreeMap::new(),
            generated_ops: Vec::new(),
            adapter_state: Arc::new(Mutex::new(FakeAdapterState::default())),
        }
    }
}

impl Default for FakeDialect {
    fn default() -> Self {
        Self::new()
    }
}

#[allow(dead_code)]
impl FakeDialect {
    pub fn new() -> Self {
        Self::with_policy(&DEFAULT_EQUIVALENCE_POLICY)
    }

    pub fn with_policy(policy: &'static dyn EquivalencePolicy) -> Self {
        Self {
            state: Arc::new(Mutex::new(FakeDialectState::default())),
            policy,
        }
    }

    pub fn set_export_schema_sql(&self, sql: impl Into<String>) {
        let state = self.state.lock().expect("fake dialect mutex should lock");
        state
            .adapter_state
            .lock()
            .expect("fake adapter mutex should lock")
            .export_schema_sql = sql.into();
    }

    pub fn set_schema_search_path(&self, search_path: Vec<String>) {
        let state = self.state.lock().expect("fake dialect mutex should lock");
        state
            .adapter_state
            .lock()
            .expect("fake adapter mutex should lock")
            .schema_search_path = search_path;
    }

    pub fn set_parse_result(&self, sql: impl Into<String>, objects: Vec<SchemaObject>) {
        self.state
            .lock()
            .expect("fake dialect mutex should lock")
            .parse_results
            .insert(sql.into(), objects);
    }

    pub fn generated_ops(&self) -> Vec<DiffOp> {
        self.state
            .lock()
            .expect("fake dialect mutex should lock")
            .generated_ops
            .clone()
    }

    pub fn executed_sql(&self) -> Vec<String> {
        let state = self.state.lock().expect("fake dialect mutex should lock");
        state
            .adapter_state
            .lock()
            .expect("fake adapter mutex should lock")
            .executed_sql
            .clone()
    }
}

impl Dialect for FakeDialect {
    fn name(&self) -> &str {
        "fake"
    }

    fn parse(&self, sql: &str) -> Result<Vec<SchemaObject>> {
        self.state
            .lock()
            .expect("fake dialect mutex should lock")
            .parse_results
            .get(sql)
            .cloned()
            .ok_or_else(|| missing_parse_fixture(sql))
    }

    fn generate_ddl(&self, ops: &[DiffOp]) -> Result<Vec<Statement>> {
        self.state
            .lock()
            .expect("fake dialect mutex should lock")
            .generated_ops = ops.to_vec();

        Ok(ops
            .iter()
            .map(|op| Statement::Sql {
                sql: diff_op_to_sql(op),
                transactional: true,
                context: None,
            })
            .collect())
    }

    fn to_sql(&self, obj: &SchemaObject) -> Result<String> {
        Ok(format!("{obj:?}"))
    }

    fn normalize(&self, _obj: &mut SchemaObject) {}

    fn equivalence_policy(&self) -> &'static dyn EquivalencePolicy {
        self.policy
    }

    fn quote_ident(&self, ident: &Ident) -> String {
        format!("\"{}\"", ident.value)
    }

    fn batch_separator(&self) -> &str {
        ""
    }

    fn connect(&self, _config: &ConnectionConfig) -> Result<Box<dyn DatabaseAdapter>> {
        let adapter_state = Arc::clone(
            &self
                .state
                .lock()
                .expect("fake dialect mutex should lock")
                .adapter_state,
        );
        Ok(Box::new(FakeAdapter::new(adapter_state)))
    }
}

fn diff_op_to_sql(op: &DiffOp) -> String {
    match op {
        DiffOp::CreateTable(table) => format!("CREATE TABLE {};", table.name.name.value),
        DiffOp::DropTable(name) => format!("DROP TABLE {};", name.name.value),
        DiffOp::AlterColumn { table, column, .. } => {
            format!(
                "ALTER TABLE {} ALTER COLUMN {};",
                table.name.value, column.value
            )
        }
        _ => format!("{op:?};"),
    }
}

fn missing_parse_fixture(sql: &str) -> stateql_core::Error {
    ParseError::StatementConversion {
        statement_index: 0,
        source_sql: sql.to_string(),
        source_location: None,
        source: boxed_error(format!("missing fake parse fixture for SQL: {sql}")),
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

fn boxed_error(message: impl Into<String>) -> Box<dyn StdError + Send + Sync> {
    Box::new(FakeSourceError(message.into()))
}

pub fn test_connection_config() -> ConnectionConfig {
    ConnectionConfig {
        host: None,
        port: None,
        user: None,
        password: None,
        database: "stateql_test".to_string(),
        socket: None,
        extra: BTreeMap::new(),
    }
}
