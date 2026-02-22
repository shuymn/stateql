use crate::{DatabaseAdapter, Result, Statement, Transaction};

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
            Statement::BatchBoundary => Ok(start + 1),
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
                } => break,
                Statement::BatchBoundary => {
                    cursor += 1;
                }
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
