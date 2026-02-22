use std::{
    error::Error as StdError,
    fmt::Write as _,
    io,
    sync::{Mutex, MutexGuard},
};

use postgres::{Client, NoTls, Row, types::FromSqlOwned};
use stateql_core::{
    ConnectionConfig, DatabaseAdapter, ExecutionError, Result, Transaction, Version,
};

use crate::export_queries;

const BEGIN_SQL: &str = "BEGIN";
const CONNECT_SQL: &str = "CONNECT postgres";
const DEFAULT_POSTGRES_HOST: &str = "127.0.0.1";
const DEFAULT_POSTGRES_SCHEMA: &str = "public";
const MINIMUM_POSTGRES_MAJOR_VERSION: u16 = 13;
const SERVER_VERSION_OVERRIDE_KEY: &str = "postgres.server_version";
const SEARCH_PATH_OVERRIDE_KEY: &str = "postgres.search_path";
const POISONED_CLIENT_MESSAGE: &str = "postgres connection state was poisoned";

pub(crate) struct PostgresAdapter {
    client: Mutex<Client>,
    search_path: Vec<String>,
    server_version: Version,
}

struct TableRow {
    schema: String,
    name: String,
    partition_key: Option<String>,
    access_method: Option<String>,
    tablespace: Option<String>,
}

struct ColumnRow {
    name: String,
    data_type: String,
    not_null: bool,
    default_expr: Option<String>,
    identity_generation: Option<String>,
}

struct PartitionChildRow {
    schema: String,
    name: String,
    parent_schema: String,
    parent_name: String,
    bound: String,
}

pub(crate) fn connect(config: &ConnectionConfig) -> Result<Box<dyn DatabaseAdapter>> {
    if let Some(raw_version) = config.extra.get(SERVER_VERSION_OVERRIDE_KEY) {
        let version = parse_server_version(raw_version)
            .ok_or_else(|| invalid_server_version_error(raw_version))?;
        ensure_minimum_version(&version, raw_version)?;
    }

    let mut client = connect_client(config)?;
    let server_version_raw =
        if let Some(raw_version) = config.extra.get(SERVER_VERSION_OVERRIDE_KEY) {
            raw_version.clone()
        } else {
            query_scalar(&mut client, export_queries::SHOW_SERVER_VERSION_QUERY)?
        };
    let server_version = parse_server_version(&server_version_raw)
        .ok_or_else(|| invalid_server_version_error(&server_version_raw))?;
    ensure_minimum_version(&server_version, &server_version_raw)?;

    let search_path_raw = if let Some(raw_search_path) = config.extra.get(SEARCH_PATH_OVERRIDE_KEY)
    {
        raw_search_path.clone()
    } else {
        query_scalar(&mut client, export_queries::SHOW_SEARCH_PATH_QUERY)?
    };
    let mut search_path = parse_search_path(&search_path_raw);
    if search_path.is_empty() {
        search_path.push(DEFAULT_POSTGRES_SCHEMA.to_string());
    }

    Ok(Box::new(PostgresAdapter {
        client: Mutex::new(client),
        search_path,
        server_version,
    }))
}

impl PostgresAdapter {
    fn lock_client(&self, sql: &str) -> Result<MutexGuard<'_, Client>> {
        self.client
            .lock()
            .map_err(|_| execution_error(sql, io::Error::other(POISONED_CLIENT_MESSAGE)))
    }
}

impl DatabaseAdapter for PostgresAdapter {
    fn export_schema(&self) -> Result<String> {
        let mut client = self.lock_client(export_queries::TABLE_NAMES_QUERY)?;
        let table_rows = client
            .query(export_queries::TABLE_NAMES_QUERY, &[])
            .map_err(|source| execution_error(export_queries::TABLE_NAMES_QUERY, source))?;
        let tables = table_rows
            .iter()
            .map(decode_table_row)
            .collect::<Result<Vec<_>>>()?;

        let mut statements = Vec::new();

        for table in &tables {
            let column_rows = client
                .query(
                    export_queries::TABLE_COLUMNS_QUERY,
                    &[&table.schema, &table.name],
                )
                .map_err(|source| execution_error(export_queries::TABLE_COLUMNS_QUERY, source))?;
            let columns = column_rows
                .iter()
                .map(decode_column_row)
                .collect::<Result<Vec<_>>>()?;
            statements.push(render_create_table(table, &columns));
        }

        let partition_rows = client
            .query(export_queries::PARTITION_CHILD_TABLES_QUERY, &[])
            .map_err(|source| {
                execution_error(export_queries::PARTITION_CHILD_TABLES_QUERY, source)
            })?;
        let partition_children = partition_rows
            .iter()
            .map(decode_partition_row)
            .collect::<Result<Vec<_>>>()?;
        for partition_child in &partition_children {
            statements.push(render_partition_child_table(partition_child));
        }

        Ok(statements.join("\n\n"))
    }

    fn execute(&self, sql: &str) -> Result<()> {
        let mut client = self.lock_client(sql)?;
        client
            .batch_execute(sql)
            .map_err(|source| execution_error(sql, source))
    }

    fn begin(&mut self) -> Result<Transaction<'_>> {
        self.execute(BEGIN_SQL)?;
        Ok(Transaction::new(self))
    }

    fn schema_search_path(&self) -> Vec<String> {
        self.search_path.clone()
    }

    fn server_version(&self) -> Result<Version> {
        Ok(self.server_version.clone())
    }
}

pub(crate) fn parse_search_path(raw: &str) -> Vec<String> {
    split_search_path(raw)
        .into_iter()
        .filter_map(|entry| normalize_search_path_entry(entry.as_str()))
        .collect()
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

fn connect_client(config: &ConnectionConfig) -> Result<Client> {
    let mut postgres_config = postgres::Config::new();

    if let Some(socket_path) = &config.socket {
        postgres_config.host_path(socket_path);
    } else if let Some(host) = &config.host {
        postgres_config.host(host);
    } else {
        postgres_config.host(DEFAULT_POSTGRES_HOST);
    }

    if let Some(port) = config.port {
        postgres_config.port(port);
    }
    if let Some(user) = &config.user {
        postgres_config.user(user);
    }
    if let Some(password) = &config.password {
        postgres_config.password(password);
    }
    postgres_config.dbname(&config.database);

    postgres_config
        .connect(NoTls)
        .map_err(|source| execution_error(CONNECT_SQL, source))
}

fn query_scalar(client: &mut Client, sql: &str) -> Result<String> {
    let row = client
        .query_one(sql, &[])
        .map_err(|source| execution_error(sql, source))?;
    row.try_get::<_, String>(0)
        .map_err(|source| execution_error(sql, source))
}

fn decode_table_row(row: &Row) -> Result<TableRow> {
    Ok(TableRow {
        schema: row_value(row, "table_schema", export_queries::TABLE_NAMES_QUERY)?,
        name: row_value(row, "table_name", export_queries::TABLE_NAMES_QUERY)?,
        partition_key: row_value(row, "partition_key", export_queries::TABLE_NAMES_QUERY)?,
        access_method: row_value(row, "access_method", export_queries::TABLE_NAMES_QUERY)?,
        tablespace: row_value(row, "tablespace_name", export_queries::TABLE_NAMES_QUERY)?,
    })
}

fn decode_column_row(row: &Row) -> Result<ColumnRow> {
    Ok(ColumnRow {
        name: row_value(row, "column_name", export_queries::TABLE_COLUMNS_QUERY)?,
        data_type: row_value(row, "data_type", export_queries::TABLE_COLUMNS_QUERY)?,
        not_null: row_value(row, "not_null", export_queries::TABLE_COLUMNS_QUERY)?,
        default_expr: row_value(row, "default_expr", export_queries::TABLE_COLUMNS_QUERY)?,
        identity_generation: row_value(
            row,
            "identity_generation",
            export_queries::TABLE_COLUMNS_QUERY,
        )?,
    })
}

fn decode_partition_row(row: &Row) -> Result<PartitionChildRow> {
    Ok(PartitionChildRow {
        schema: row_value(
            row,
            "partition_schema",
            export_queries::PARTITION_CHILD_TABLES_QUERY,
        )?,
        name: row_value(
            row,
            "partition_name",
            export_queries::PARTITION_CHILD_TABLES_QUERY,
        )?,
        parent_schema: row_value(
            row,
            "parent_schema",
            export_queries::PARTITION_CHILD_TABLES_QUERY,
        )?,
        parent_name: row_value(
            row,
            "parent_name",
            export_queries::PARTITION_CHILD_TABLES_QUERY,
        )?,
        bound: row_value(
            row,
            "partition_bound",
            export_queries::PARTITION_CHILD_TABLES_QUERY,
        )?,
    })
}

fn render_create_table(table: &TableRow, columns: &[ColumnRow]) -> String {
    let mut sql = String::new();
    write!(
        sql,
        "CREATE TABLE {}",
        render_qualified_name(table.schema.as_str(), table.name.as_str())
    )
    .expect("writing to String should not fail");

    if columns.is_empty() {
        sql.push_str(" ()");
    } else {
        sql.push_str(" (\n");
        for (index, column) in columns.iter().enumerate() {
            if index > 0 {
                sql.push_str(",\n");
            }
            write!(sql, "  {}", render_column(column)).expect("writing to String should not fail");
        }
        sql.push_str("\n)");
    }

    if let Some(partition_key) = table
        .partition_key
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        write!(sql, " PARTITION BY {partition_key}").expect("writing to String should not fail");
    }
    if let Some(access_method) = table
        .access_method
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty() && *value != "heap")
    {
        write!(sql, " USING {}", quote_identifier(access_method))
            .expect("writing to String should not fail");
    }
    if let Some(tablespace) = table
        .tablespace
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        write!(sql, " TABLESPACE {}", quote_identifier(tablespace))
            .expect("writing to String should not fail");
    }

    sql.push(';');
    sql
}

fn render_column(column: &ColumnRow) -> String {
    let mut sql = format!(
        "{} {}",
        quote_identifier(column.name.as_str()),
        column.data_type.as_str()
    );

    if column.not_null {
        sql.push_str(" NOT NULL");
    }
    if let Some(default_expr) = column
        .default_expr
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        write!(sql, " DEFAULT {default_expr}").expect("writing to String should not fail");
    }
    if let Some(identity_generation) = column
        .identity_generation
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        write!(sql, " GENERATED {identity_generation} AS IDENTITY")
            .expect("writing to String should not fail");
    }

    sql
}

fn render_partition_child_table(partition_child: &PartitionChildRow) -> String {
    let mut sql = format!(
        "CREATE TABLE {} PARTITION OF {}",
        render_qualified_name(
            partition_child.schema.as_str(),
            partition_child.name.as_str()
        ),
        render_qualified_name(
            partition_child.parent_schema.as_str(),
            partition_child.parent_name.as_str(),
        )
    );

    let bound = partition_child.bound.trim();
    if !bound.is_empty() {
        write!(sql, " {bound}").expect("writing to String should not fail");
    }
    sql.push(';');

    sql
}

fn render_qualified_name(schema: &str, name: &str) -> String {
    format!("{}.{}", quote_identifier(schema), quote_identifier(name))
}

fn quote_identifier(identifier: &str) -> String {
    format!("\"{}\"", identifier.replace('"', "\"\""))
}

fn normalize_search_path_entry(entry: &str) -> Option<String> {
    let trimmed = entry.trim();
    if trimmed.is_empty() {
        return None;
    }

    let normalized = unquote_search_path_entry(trimmed);
    if is_implicit_schema(normalized.as_str()) {
        return None;
    }

    Some(normalized)
}

fn split_search_path(raw: &str) -> Vec<String> {
    let mut entries = Vec::new();
    let mut current = String::new();
    let mut chars = raw.chars().peekable();
    let mut in_quotes = false;

    while let Some(ch) = chars.next() {
        match ch {
            '"' => {
                current.push(ch);
                if in_quotes && chars.peek() == Some(&'"') {
                    current.push('"');
                    let _ = chars.next();
                } else {
                    in_quotes = !in_quotes;
                }
            }
            ',' if !in_quotes => {
                entries.push(current.trim().to_string());
                current.clear();
            }
            _ => current.push(ch),
        }
    }

    if !current.is_empty() || raw.ends_with(',') {
        entries.push(current.trim().to_string());
    }

    entries
}

fn unquote_search_path_entry(entry: &str) -> String {
    if entry.len() >= 2 && entry.starts_with('"') && entry.ends_with('"') {
        return entry[1..entry.len() - 1].replace("\"\"", "\"");
    }
    entry.to_string()
}

fn is_implicit_schema(schema: &str) -> bool {
    let normalized = schema.trim();
    normalized.eq_ignore_ascii_case("$user")
        || normalized.eq_ignore_ascii_case("pg_catalog")
        || normalized.eq_ignore_ascii_case("pg_temp")
        || normalized.to_ascii_lowercase().starts_with("pg_temp_")
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
    if version.major >= MINIMUM_POSTGRES_MAJOR_VERSION {
        return Ok(());
    }

    Err(execution_error(
        export_queries::SHOW_SERVER_VERSION_QUERY,
        io::Error::other(format!(
            "postgres server version `{raw_version}` is not supported; requires {MINIMUM_POSTGRES_MAJOR_VERSION}+"
        )),
    ))
}

fn invalid_server_version_error(raw_version: &str) -> stateql_core::Error {
    execution_error(
        export_queries::SHOW_SERVER_VERSION_QUERY,
        io::Error::other(format!(
            "failed to parse postgres server version string: `{raw_version}`"
        )),
    )
}

fn row_value<T>(row: &Row, column: &str, sql: &str) -> Result<T>
where
    T: FromSqlOwned,
{
    row.try_get(column)
        .map_err(|source| execution_error(sql, source))
}

fn execution_error<E>(sql: &str, source: E) -> stateql_core::Error
where
    E: StdError + Send + Sync + 'static,
{
    ExecutionError::statement_failed(0, sql, 0, None, None, source).into()
}
