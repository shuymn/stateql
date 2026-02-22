use std::{
    error::Error as StdError,
    io,
    sync::{Mutex, MutexGuard},
};

use rusqlite::Connection;
use stateql_core::{
    ConnectionConfig, DatabaseAdapter, ExecutionError, Result, Transaction, Version,
};

use crate::export_queries;

const BEGIN_SQL: &str = "BEGIN";
const CONNECT_SQL: &str = "CONNECT sqlite";
const DEFAULT_SQLITE_SCHEMA: &str = "main";
const MINIMUM_SQLITE_MAJOR_VERSION: u16 = 3;
const MINIMUM_SQLITE_MINOR_VERSION: u16 = 35;
const SERVER_VERSION_OVERRIDE_KEY: &str = "sqlite.server_version";
const POISONED_CONNECTION_MESSAGE: &str = "sqlite connection state was poisoned";

pub(crate) struct SqliteAdapter {
    connection: Mutex<Connection>,
    server_version: Version,
}

pub(crate) fn connect(config: &ConnectionConfig) -> Result<Box<dyn DatabaseAdapter>> {
    if let Some(raw_version) = config.extra.get(SERVER_VERSION_OVERRIDE_KEY) {
        let version = parse_server_version(raw_version)
            .ok_or_else(|| invalid_server_version_error(raw_version))?;
        ensure_minimum_version(&version, raw_version)?;
    }

    let connection = Connection::open(config.database.as_str())
        .map_err(|source| execution_error(CONNECT_SQL, source))?;

    let server_version_raw =
        if let Some(raw_version) = config.extra.get(SERVER_VERSION_OVERRIDE_KEY) {
            raw_version.clone()
        } else {
            query_server_version(&connection)?
        };
    let server_version = parse_server_version(&server_version_raw)
        .ok_or_else(|| invalid_server_version_error(&server_version_raw))?;
    ensure_minimum_version(&server_version, &server_version_raw)?;

    Ok(Box::new(SqliteAdapter {
        connection: Mutex::new(connection),
        server_version,
    }))
}

impl SqliteAdapter {
    fn lock_connection(&self, sql: &str) -> Result<MutexGuard<'_, Connection>> {
        self.connection
            .lock()
            .map_err(|_| execution_error(sql, io::Error::other(POISONED_CONNECTION_MESSAGE)))
    }
}

impl DatabaseAdapter for SqliteAdapter {
    fn export_schema(&self) -> Result<String> {
        let connection = self.lock_connection(export_queries::TABLE_NAMES_QUERY)?;
        let table_names = query_string_rows(&connection, export_queries::TABLE_NAMES_QUERY)?;

        let mut statements = Vec::new();

        for table_name in table_names {
            let table_sql: String = connection
                .query_row(export_queries::TABLE_DDL_QUERY, [&table_name], |row| {
                    row.get(0)
                })
                .map_err(|source| execution_error(export_queries::TABLE_DDL_QUERY, source))?;
            statements.push(ensure_statement_terminated(table_sql));
        }

        statements.extend(query_sql_statements(
            &connection,
            export_queries::VIEW_DDLS_QUERY,
        )?);
        statements.extend(query_sql_statements(
            &connection,
            export_queries::INDEX_DDLS_QUERY,
        )?);
        statements.extend(query_sql_statements(
            &connection,
            export_queries::TRIGGER_DDLS_QUERY,
        )?);

        Ok(statements.join("\n\n"))
    }

    fn execute(&self, sql: &str) -> Result<()> {
        let connection = self.lock_connection(sql)?;
        connection
            .execute_batch(sql)
            .map_err(|source| execution_error(sql, source))
    }

    fn begin(&mut self) -> Result<Transaction<'_>> {
        self.execute(BEGIN_SQL)?;
        Ok(Transaction::new(self))
    }

    fn schema_search_path(&self) -> Vec<String> {
        vec![DEFAULT_SQLITE_SCHEMA.to_string()]
    }

    fn server_version(&self) -> Result<Version> {
        Ok(self.server_version.clone())
    }
}

pub(crate) fn parse_server_version(raw: &str) -> Option<Version> {
    let mut parts = raw.split_whitespace().next()?.split('.');
    let major = parse_version_component(parts.next()?)?;
    let minor = parts.next().and_then(parse_version_component).unwrap_or(0);
    let patch = parts.next().and_then(parse_version_component).unwrap_or(0);

    Some(Version {
        major,
        minor,
        patch,
    })
}

fn query_server_version(connection: &Connection) -> Result<String> {
    connection
        .query_row(export_queries::SHOW_SERVER_VERSION_QUERY, [], |row| {
            row.get(0)
        })
        .map_err(|source| execution_error(export_queries::SHOW_SERVER_VERSION_QUERY, source))
}

fn query_string_rows(connection: &Connection, query: &str) -> Result<Vec<String>> {
    let mut statement = connection
        .prepare(query)
        .map_err(|source| execution_error(query, source))?;
    let mut rows = statement
        .query([])
        .map_err(|source| execution_error(query, source))?;

    let mut values = Vec::new();
    while let Some(row) = rows
        .next()
        .map_err(|source| execution_error(query, source))?
    {
        values.push(
            row.get::<_, String>(0)
                .map_err(|source| execution_error(query, source))?,
        );
    }

    Ok(values)
}

fn query_sql_statements(connection: &Connection, query: &str) -> Result<Vec<String>> {
    query_string_rows(connection, query)
        .map(|rows| rows.into_iter().map(ensure_statement_terminated).collect())
}

fn ensure_statement_terminated(sql: String) -> String {
    let trimmed = sql.trim();
    if trimmed.ends_with(';') {
        trimmed.to_string()
    } else {
        format!("{trimmed};")
    }
}

fn parse_version_component(raw: &str) -> Option<u16> {
    let digits = raw
        .chars()
        .take_while(|ch| ch.is_ascii_digit())
        .collect::<String>();
    if digits.is_empty() {
        return None;
    }
    digits.parse::<u16>().ok()
}

fn ensure_minimum_version(version: &Version, raw_version: &str) -> Result<()> {
    if version.major > MINIMUM_SQLITE_MAJOR_VERSION
        || (version.major == MINIMUM_SQLITE_MAJOR_VERSION
            && version.minor >= MINIMUM_SQLITE_MINOR_VERSION)
    {
        return Ok(());
    }

    Err(execution_error(
        export_queries::SHOW_SERVER_VERSION_QUERY,
        io::Error::other(format!(
            "sqlite server version `{raw_version}` is not supported; requires {MINIMUM_SQLITE_MAJOR_VERSION}.{MINIMUM_SQLITE_MINOR_VERSION}+"
        )),
    ))
}

fn invalid_server_version_error(raw_version: &str) -> stateql_core::Error {
    execution_error(
        export_queries::SHOW_SERVER_VERSION_QUERY,
        io::Error::other(format!(
            "failed to parse sqlite server version string: `{raw_version}`"
        )),
    )
}

fn execution_error<E>(sql: &str, source: E) -> stateql_core::Error
where
    E: StdError + Send + Sync + 'static,
{
    ExecutionError::statement_failed(0, sql, 0, None, None, source).into()
}
