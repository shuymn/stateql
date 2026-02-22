use crate::{Result, Statement};

pub trait DatabaseAdapter {
    fn export_schema(&self) -> Result<String>;
    fn execute(&mut self, statement: &Statement) -> Result<()>;
}
