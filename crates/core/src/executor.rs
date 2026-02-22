use crate::{
    DatabaseAdapter, Error, ExecutionError, Result, SourceLocation, Statement, StatementContext,
    Transaction,
};

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
        let mut executed_statements = 0;
        while index < statements.len() {
            index = self.execute_next_group(statements, index, &mut executed_statements)?;
        }

        Ok(())
    }

    fn execute_next_group(
        &mut self,
        statements: &[Statement],
        start: usize,
        executed_statements: &mut usize,
    ) -> Result<usize> {
        match &statements[start] {
            Statement::Sql {
                transactional: true,
                sql,
                context,
                ..
            } => self.execute_transactional_group(
                statements,
                start,
                sql,
                context.as_ref(),
                executed_statements,
            ),
            Statement::Sql {
                sql,
                transactional: false,
                context,
            } => self.execute_non_transactional_statement(
                start,
                sql,
                context.as_ref(),
                executed_statements,
            ),
            Statement::BatchBoundary => Ok(start + 1),
        }
    }

    fn execute_transactional_group(
        &mut self,
        statements: &[Statement],
        start: usize,
        start_sql: &str,
        start_context: Option<&StatementContext>,
        executed_statements: &mut usize,
    ) -> Result<usize> {
        let mut tx = Some(self.adapter.begin().map_err(|source| {
            Self::build_statement_failed(
                start,
                start_sql,
                start_context,
                *executed_statements,
                source,
            )
        })?);
        let mut cursor = start;
        let mut last_sql = start_sql;
        let mut last_statement_index = start;
        let mut last_context = start_context.cloned();

        while let Some(statement) = statements.get(cursor) {
            match statement {
                Statement::Sql {
                    sql,
                    transactional: true,
                    context,
                } => {
                    if let Some(transaction) = tx.as_mut() {
                        transaction.execute(sql).map_err(|source| {
                            Self::build_statement_failed(
                                cursor,
                                sql,
                                context.as_ref(),
                                *executed_statements,
                                source,
                            )
                        })?;
                    }
                    *executed_statements += 1;
                    last_statement_index = cursor;
                    last_sql = sql;
                    last_context = context.clone();
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

        Self::flush_tx_if_open(tx).map_err(|source| {
            Self::build_statement_failed(
                last_statement_index,
                last_sql,
                last_context.as_ref(),
                *executed_statements,
                source,
            )
        })?;
        Ok(cursor)
    }

    fn execute_non_transactional_statement(
        &mut self,
        start: usize,
        sql: &str,
        context: Option<&StatementContext>,
        executed_statements: &mut usize,
    ) -> Result<usize> {
        self.adapter.execute(sql).map_err(|source| {
            Self::build_statement_failed(start, sql, context, *executed_statements, source)
        })?;
        *executed_statements += 1;
        Ok(start + 1)
    }

    fn flush_tx_if_open(transaction: Option<Transaction<'_>>) -> Result<()> {
        if let Some(transaction) = transaction {
            transaction.commit()?;
        }

        Ok(())
    }

    fn build_statement_failed(
        statement_index: usize,
        sql: &str,
        context: Option<&StatementContext>,
        executed_statements: usize,
        source: Error,
    ) -> Error {
        let source_location = Self::inherited_source_location(&source);
        let inherited_context = Self::inherited_statement_context(&source);

        ExecutionError::statement_failed(
            statement_index,
            sql,
            executed_statements,
            source_location,
            context.cloned().or(inherited_context),
            source,
        )
        .into()
    }

    fn inherited_source_location(source: &Error) -> Option<SourceLocation> {
        match source {
            Error::Execute(ExecutionError::StatementFailed {
                source_location, ..
            }) => source_location.clone(),
            _ => None,
        }
    }

    fn inherited_statement_context(source: &Error) -> Option<StatementContext> {
        match source {
            Error::Execute(ExecutionError::StatementFailed {
                statement_context, ..
            }) => statement_context.as_deref().cloned(),
            _ => None,
        }
    }
}
