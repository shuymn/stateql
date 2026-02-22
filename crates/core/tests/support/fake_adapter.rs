use std::{cell::RefCell, error::Error as StdError, fmt};

use stateql_core::{DatabaseAdapter, ExecutionError, Result, SourceLocation, Transaction, Version};

pub const BEGIN_SQL: &str = "BEGIN";
pub const COMMIT_SQL: &str = "COMMIT";
pub const ROLLBACK_SQL: &str = "ROLLBACK";

#[derive(Debug, Default)]
struct FailureRule {
    sql: String,
    message: String,
}

#[derive(Debug)]
pub struct FakeAdapter {
    state: RefCell<FakeAdapterState>,
}

#[derive(Debug)]
struct FakeAdapterState {
    export_schema_sql: String,
    schema_search_path: Vec<String>,
    server_version: Version,
    executed_sql: Vec<String>,
    begin_count: usize,
    commit_count: usize,
    rollback_count: usize,
    fail_on_sql: Option<FailureRule>,
}

impl Default for FakeAdapterState {
    fn default() -> Self {
        Self {
            export_schema_sql: String::new(),
            schema_search_path: vec!["public".to_string()],
            server_version: Version {
                major: 0,
                minor: 0,
                patch: 0,
            },
            executed_sql: Vec::new(),
            begin_count: 0,
            commit_count: 0,
            rollback_count: 0,
            fail_on_sql: None,
        }
    }
}

impl Default for FakeAdapter {
    fn default() -> Self {
        Self {
            state: RefCell::new(FakeAdapterState::default()),
        }
    }
}

#[allow(dead_code)]
impl FakeAdapter {
    pub fn set_export_schema_sql(&self, sql: impl Into<String>) {
        self.state.borrow_mut().export_schema_sql = sql.into();
    }

    pub fn set_schema_search_path(&self, search_path: Vec<String>) {
        self.state.borrow_mut().schema_search_path = search_path;
    }

    pub fn set_server_version(&self, version: Version) {
        self.state.borrow_mut().server_version = version;
    }

    pub fn set_fail_on_sql(&self, sql: impl Into<String>, message: impl Into<String>) {
        self.state.borrow_mut().fail_on_sql = Some(FailureRule {
            sql: sql.into(),
            message: message.into(),
        });
    }

    pub fn clear_fail_on_sql(&self) {
        self.state.borrow_mut().fail_on_sql = None;
    }

    pub fn executed_sql(&self) -> Vec<String> {
        self.state.borrow().executed_sql.clone()
    }

    pub fn begin_count(&self) -> usize {
        self.state.borrow().begin_count
    }

    pub fn commit_count(&self) -> usize {
        self.state.borrow().commit_count
    }

    pub fn rollback_count(&self) -> usize {
        self.state.borrow().rollback_count
    }
}

impl DatabaseAdapter for FakeAdapter {
    fn export_schema(&self) -> Result<String> {
        Ok(self.state.borrow().export_schema_sql.clone())
    }

    fn execute(&self, sql: &str) -> Result<()> {
        let mut state = self.state.borrow_mut();

        if let Some(rule) = &state.fail_on_sql
            && rule.sql == sql
        {
            return Err(ExecutionError::StatementFailed {
                statement_index: state.executed_sql.len(),
                sql: sql.to_string(),
                executed_statements: state.executed_sql.len(),
                source_location: Some(SourceLocation {
                    line: 1,
                    column: None,
                }),
                source: boxed_error(rule.message.clone()),
            }
            .into());
        }

        state.executed_sql.push(sql.to_string());
        match sql {
            BEGIN_SQL => state.begin_count += 1,
            COMMIT_SQL => state.commit_count += 1,
            ROLLBACK_SQL => state.rollback_count += 1,
            _ => {}
        }

        Ok(())
    }

    fn begin(&mut self) -> Result<Transaction<'_>> {
        self.execute(BEGIN_SQL)?;
        Ok(Transaction::new(self))
    }

    fn schema_search_path(&self) -> Vec<String> {
        self.state.borrow().schema_search_path.clone()
    }

    fn server_version(&self) -> Result<Version> {
        Ok(self.state.borrow().server_version.clone())
    }
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
