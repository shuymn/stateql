use crate::{Result, Version};

const COMMIT_SQL: &str = "COMMIT";
const ROLLBACK_SQL: &str = "ROLLBACK";

/// A single database connection used by the execution pipeline.
///
/// Implementations must keep all methods on the same physical connection.
/// This trait intentionally exposes synchronous I/O only; async boundaries
/// must stay inside adapter implementations and not leak into core APIs.
pub trait DatabaseAdapter: Send {
    fn export_schema(&self) -> Result<String>;
    fn execute(&self, sql: &str) -> Result<()>;
    fn begin(&mut self) -> Result<Transaction<'_>>;
    fn schema_search_path(&self) -> Vec<String>;
    fn server_version(&self) -> Result<Version>;
}

/// RAII transaction handle.
///
/// If dropped without calling `commit`, it triggers `ROLLBACK` on
/// the same adapter connection.
pub struct Transaction<'a> {
    adapter: &'a mut dyn DatabaseAdapter,
    committed: bool,
}

impl<'a> Transaction<'a> {
    pub fn new(adapter: &'a mut dyn DatabaseAdapter) -> Self {
        Self {
            adapter,
            committed: false,
        }
    }

    pub fn execute(&mut self, sql: &str) -> Result<()> {
        self.adapter.execute(sql)
    }

    pub fn commit(mut self) -> Result<()> {
        self.adapter.execute(COMMIT_SQL)?;
        self.committed = true;
        Ok(())
    }
}

impl Drop for Transaction<'_> {
    fn drop(&mut self) {
        if !self.committed {
            let _ = self.adapter.execute(ROLLBACK_SQL);
        }
    }
}
