use std::{error::Error as StdError, io::ErrorKind};

use crate::{DatabaseAdapter, ExecutionError, Result, Statement, Transaction};

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
            } => self.execute_non_transactional_statement(start, sql),
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
        let mut tx = Some(self.adapter.begin()?);
        let mut cursor = start;

        while let Some(statement) = statements.get(cursor) {
            match statement {
                Statement::Sql {
                    sql,
                    transactional: true,
                    ..
                } => {
                    if let Some(transaction) = tx.as_mut() {
                        transaction.execute(sql)?;
                    }
                    cursor += 1;
                }
                Statement::Sql {
                    transactional: false,
                    ..
                }
                | Statement::BatchBoundary => break,
            }
        }

        Self::flush_tx_if_open(tx)?;
        Ok(cursor)
    }

    fn execute_non_transactional_statement(&mut self, start: usize, sql: &str) -> Result<usize> {
        self.adapter.execute(sql)?;
        Ok(start + 1)
    }

    fn flush_tx_if_open(transaction: Option<Transaction<'_>>) -> Result<()> {
        if let Some(transaction) = transaction {
            transaction.commit()?;
        }

        Ok(())
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
