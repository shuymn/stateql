use crate::{CoreResult, Statement};

pub trait DatabaseAdapter {
    fn export_schema(&self) -> CoreResult<String>;
    fn execute(&mut self, statement: &Statement) -> CoreResult<()>;
}
