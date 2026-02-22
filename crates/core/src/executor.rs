use std::{error::Error as StdError, io::ErrorKind};

use crate::{DatabaseAdapter, ExecutionError, Result, Statement};

pub struct Executor<'a> {
    adapter: &'a mut dyn DatabaseAdapter,
}

impl<'a> Executor<'a> {
    #[must_use]
    pub fn new(adapter: &'a mut dyn DatabaseAdapter) -> Self {
        Self { adapter }
    }

    pub fn execute_plan(&mut self, statements: &[Statement]) -> Result<()> {
        let mut index = 0;
        while index < statements.len() {
            index = self.execute_next_group(statements, index)?;
        }

        Ok(())
    }

    fn execute_next_group(&mut self, statements: &[Statement], start: usize) -> Result<usize> {
        match &statements[start] {
            Statement::Sql {
                transactional: true,
                ..
            } => self.execute_transactional_group(statements, start),
            Statement::Sql {
                sql,
                transactional: false,
                ..
            } => Err(unsupported_statement_error(
                start,
                sql.clone(),
                "non-transactional statements are not supported yet",
            )
            .into()),
            Statement::BatchBoundary => Err(unsupported_statement_error(
                start,
                "<batch-boundary>".to_string(),
                "batch boundaries are not supported yet",
            )
            .into()),
        }
    }

    fn execute_transactional_group(
        &mut self,
        statements: &[Statement],
        start: usize,
    ) -> Result<usize> {
        let mut tx = self.adapter.begin()?;
        let mut cursor = start;

        while let Some(statement) = statements.get(cursor) {
            match statement {
                Statement::Sql {
                    sql,
                    transactional: true,
                    ..
                } => {
                    tx.execute(sql)?;
                    cursor += 1;
                }
                Statement::Sql {
                    transactional: false,
                    ..
                }
                | Statement::BatchBoundary => break,
            }
        }

        tx.commit()?;
        Ok(cursor)
    }
}

fn unsupported_statement_error(
    statement_index: usize,
    sql: String,
    message: &str,
) -> ExecutionError {
    ExecutionError::StatementFailed {
        statement_index,
        sql,
        executed_statements: statement_index,
        source_location: None,
        source: boxed_invalid_input_error(message),
    }
}

fn boxed_invalid_input_error(message: &str) -> Box<dyn StdError + Send + Sync> {
    Box::new(std::io::Error::new(ErrorKind::InvalidInput, message))
}
