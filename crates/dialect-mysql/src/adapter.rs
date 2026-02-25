use std::{
    error::Error as StdError,
    io,
    sync::{Mutex, MutexGuard},
};

use mysql::{OptsBuilder, Pool, PooledConn, Row, prelude::Queryable};
use stateql_core::{
    ConnectionConfig, DatabaseAdapter, ExecutionError, Result, Transaction, Version,
};

use crate::export_queries;

const BEGIN_SQL: &str = "BEGIN";
const CONNECT_SQL: &str = "CONNECT mysql";
const DEFAULT_MYSQL_HOST: &str = "127.0.0.1";
const DEFAULT_MYSQL_PORT: u16 = 3306;
const MINIMUM_MYSQL_MAJOR_VERSION: u16 = 8;
const MINIMUM_MYSQL_MINOR_VERSION: u16 = 0;
const SERVER_VERSION_OVERRIDE_KEY: &str = "mysql.server_version";
const LOWER_CASE_TABLE_NAMES_OVERRIDE_KEY: &str = "mysql.lower_case_table_names";
const POISONED_CONNECTION_MESSAGE: &str = "mysql connection state was poisoned";

pub(crate) struct MysqlAdapter {
    connection: Mutex<PooledConn>,
    default_schema: String,
    server_version: Version,
}

pub(crate) fn connect(config: &ConnectionConfig) -> Result<Box<dyn DatabaseAdapter>> {
    if let Some(raw_version) = config.extra.get(SERVER_VERSION_OVERRIDE_KEY) {
        let version = parse_server_version(raw_version)
            .ok_or_else(|| invalid_server_version_error(raw_version))?;
        ensure_minimum_version(&version, raw_version)?;
    }

    let mut connection = connect_connection(config)?;
    let server_version_raw =
        if let Some(raw_version) = config.extra.get(SERVER_VERSION_OVERRIDE_KEY) {
            raw_version.clone()
        } else {
            query_scalar(&mut connection, export_queries::SHOW_SERVER_VERSION_QUERY)?
        };
    let server_version = parse_server_version(&server_version_raw)
        .ok_or_else(|| invalid_server_version_error(&server_version_raw))?;
    ensure_minimum_version(&server_version, &server_version_raw)?;

    let lower_case_table_names_raw = if let Some(raw_lower_case_table_names) =
        config.extra.get(LOWER_CASE_TABLE_NAMES_OVERRIDE_KEY)
    {
        raw_lower_case_table_names.clone()
    } else {
        query_lower_case_table_names(&mut connection)?
    };
    let _lower_case_table_names = parse_lower_case_table_names(&lower_case_table_names_raw)
        .ok_or_else(|| invalid_lower_case_table_names_error(&lower_case_table_names_raw))?;

    Ok(Box::new(MysqlAdapter {
        connection: Mutex::new(connection),
        default_schema: config.database.clone(),
        server_version,
    }))
}

impl MysqlAdapter {
    fn lock_connection(&self, sql: &str) -> Result<MutexGuard<'_, PooledConn>> {
        self.connection
            .lock()
            .map_err(|_| execution_error(sql, io::Error::other(POISONED_CONNECTION_MESSAGE)))
    }
}

impl DatabaseAdapter for MysqlAdapter {
    fn export_schema(&self) -> Result<String> {
        let mut connection = self.lock_connection(export_queries::TABLE_NAMES_QUERY)?;
        let table_names = query_table_names(&mut connection)?;
        let mut statements = Vec::new();

        for table_name in table_names {
            statements.push(export_table_ddl(&mut connection, &table_name)?);
        }
        statements.extend(export_views(&mut connection)?);
        statements.extend(export_triggers(&mut connection)?);

        Ok(statements.join("\n\n"))
    }

    fn execute(&self, sql: &str) -> Result<()> {
        let mut connection = self.lock_connection(sql)?;
        connection
            .query_drop(sql)
            .map_err(|source| execution_error(sql, source))
    }

    fn begin(&mut self) -> Result<Transaction<'_>> {
        self.execute(BEGIN_SQL)?;
        Ok(Transaction::new(self))
    }

    fn schema_search_path(&self) -> Vec<String> {
        vec![self.default_schema.clone()]
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

fn connect_connection(config: &ConnectionConfig) -> Result<PooledConn> {
    let mut builder = OptsBuilder::new()
        .ip_or_hostname(config.host.clone().or(Some(DEFAULT_MYSQL_HOST.to_string())))
        .tcp_port(config.port.unwrap_or(DEFAULT_MYSQL_PORT))
        .user(config.user.clone())
        .pass(config.password.clone())
        .db_name(Some(config.database.clone()));
    if let Some(socket) = &config.socket {
        builder = builder.socket(Some(socket.clone()));
    }

    let pool = Pool::new(builder).map_err(|source| execution_error(CONNECT_SQL, source))?;
    pool.get_conn()
        .map_err(|source| execution_error(CONNECT_SQL, source))
}

fn query_scalar(connection: &mut PooledConn, sql: &str) -> Result<String> {
    connection
        .query_first::<String, _>(sql)
        .map_err(|source| execution_error(sql, source))?
        .ok_or_else(|| execution_error(sql, io::Error::other("query returned no rows")))
}

fn query_lower_case_table_names(connection: &mut PooledConn) -> Result<String> {
    let query = export_queries::LOWER_CASE_TABLE_NAMES_QUERY;
    let row = connection
        .query_first::<Row, _>(query)
        .map_err(|source| execution_error(query, source))?
        .ok_or_else(|| execution_error(query, io::Error::other("query returned no rows")))?;
    row_string(&row, 1, query, "Value")
}

fn query_table_names(connection: &mut PooledConn) -> Result<Vec<String>> {
    let query = export_queries::TABLE_NAMES_QUERY;
    let rows = connection
        .query::<Row, _>(query)
        .map_err(|source| execution_error(query, source))?;
    let mut table_names = rows
        .iter()
        .map(|row| row_string(row, 0, query, "table_name"))
        .collect::<Result<Vec<_>>>()?;
    table_names.sort_unstable();
    Ok(table_names)
}

fn export_table_ddl(connection: &mut PooledConn, table_name: &str) -> Result<String> {
    let escaped_table_name = table_name.replace('`', "``");
    let query = format!("SHOW CREATE TABLE `{escaped_table_name}`");
    let row = connection
        .query_first::<Row, _>(query.as_str())
        .map_err(|source| execution_error(&query, source))?
        .ok_or_else(|| execution_error(&query, io::Error::other("query returned no rows")))?;
    let ddl = row_string(&row, 1, &query, "Create Table")?;
    Ok(ensure_statement_terminated(ddl))
}

fn export_views(connection: &mut PooledConn) -> Result<Vec<String>> {
    let query = export_queries::VIEWS_QUERY;
    let rows = connection
        .query::<Row, _>(query)
        .map_err(|source| execution_error(query, source))?;

    let mut statements = Vec::with_capacity(rows.len());
    for row in &rows {
        let view_name = row_string(row, 0, query, "TABLE_NAME")?;
        let view_definition = row_string(row, 1, query, "VIEW_DEFINITION")?;
        let security_type = row_string(row, 2, query, "SECURITY_TYPE")?;
        statements.push(format!(
            "CREATE SQL SECURITY {} VIEW {} AS {};",
            security_type.trim().to_ascii_uppercase(),
            quote_identifier(view_name.as_str()),
            view_definition.trim()
        ));
    }
    Ok(statements)
}

fn export_triggers(connection: &mut PooledConn) -> Result<Vec<String>> {
    let query = export_queries::TRIGGERS_QUERY;
    let rows = connection
        .query::<Row, _>(query)
        .map_err(|source| execution_error(query, source))?;

    let mut statements = Vec::with_capacity(rows.len());
    for row in &rows {
        let trigger_name = row_string(row, 0, query, "TRIGGER_NAME")?;
        let event = row_string(row, 1, query, "EVENT_MANIPULATION")?;
        let table = row_string(row, 2, query, "EVENT_OBJECT_TABLE")?;
        let timing = row_string(row, 3, query, "ACTION_TIMING")?;
        let statement = row_string(row, 4, query, "ACTION_STATEMENT")?;

        let body = statement.trim().trim_end_matches(';').trim();
        statements.push(format!(
            "CREATE TRIGGER {} {} {} ON {} FOR EACH ROW {};",
            quote_identifier(trigger_name.as_str()),
            timing.trim().to_ascii_uppercase(),
            event.trim().to_ascii_uppercase(),
            quote_identifier(table.as_str()),
            body
        ));
    }

    Ok(statements)
}

fn ensure_statement_terminated(sql: String) -> String {
    let trimmed = sql.trim();
    if trimmed.ends_with(';') {
        trimmed.to_string()
    } else {
        format!("{trimmed};")
    }
}

fn quote_identifier(identifier: &str) -> String {
    format!("`{}`", identifier.replace('`', "``"))
}

fn row_string(row: &Row, index: usize, query: &str, label: &str) -> Result<String> {
    row.get::<String, usize>(index).ok_or_else(|| {
        execution_error(
            query,
            io::Error::other(format!("missing column `{label}` in query result")),
        )
    })
}

fn parse_lower_case_table_names(raw: &str) -> Option<u8> {
    raw.trim().parse::<u8>().ok().filter(|value| *value <= 2)
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
    if version.major >= MINIMUM_MYSQL_MAJOR_VERSION {
        return Ok(());
    }

    Err(execution_error(
        export_queries::SHOW_SERVER_VERSION_QUERY,
        io::Error::other(format!(
            "mysql server version `{raw_version}` is not supported; requires {MINIMUM_MYSQL_MAJOR_VERSION}.{MINIMUM_MYSQL_MINOR_VERSION}+"
        )),
    ))
}

fn invalid_server_version_error(raw_version: &str) -> stateql_core::Error {
    execution_error(
        export_queries::SHOW_SERVER_VERSION_QUERY,
        io::Error::other(format!(
            "failed to parse mysql server version string: `{raw_version}`"
        )),
    )
}

fn invalid_lower_case_table_names_error(raw_value: &str) -> stateql_core::Error {
    execution_error(
        export_queries::LOWER_CASE_TABLE_NAMES_QUERY,
        io::Error::other(format!(
            "failed to parse lower_case_table_names value: `{raw_value}`"
        )),
    )
}

fn execution_error<E>(sql: &str, source: E) -> stateql_core::Error
where
    E: StdError + Send + Sync + 'static,
{
    ExecutionError::statement_failed(0, sql, 0, None, None, source).into()
}
