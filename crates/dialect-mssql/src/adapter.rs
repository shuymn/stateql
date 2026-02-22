use std::{
    error::Error as StdError,
    io,
    sync::{Mutex, MutexGuard},
};

use futures_util::TryStreamExt;
use stateql_core::{
    ConnectionConfig, DatabaseAdapter, ExecutionError, Result, Transaction, Version,
};
use tiberius::{AuthMethod, Client, Config, QueryItem};
use tokio::{
    net::TcpStream,
    runtime::{Builder, Runtime},
};
use tokio_util::compat::{Compat, TokioAsyncWriteCompatExt};

use crate::export_queries;

type TdsClient = Client<Compat<TcpStream>>;

const BEGIN_SQL: &str = "BEGIN TRANSACTION";
const CONNECT_SQL: &str = "CONNECT mssql";
const DEFAULT_MSSQL_HOST: &str = "127.0.0.1";
const DEFAULT_MSSQL_PORT: u16 = 1433;
const DEFAULT_MSSQL_SCHEMA: &str = "dbo";
const DEFAULT_MSSQL_VERSION: &str = "15.0.2000.5";
const MINIMUM_MSSQL_PRODUCT_MAJOR_VERSION: u16 = 15;
const MINIMUM_MSSQL_YEAR_VERSION: u16 = 2019;
const YEAR_VERSION_THRESHOLD: u16 = 1000;
const SERVER_VERSION_OVERRIDE_KEY: &str = "mssql.server_version";
const SCHEMA_SEARCH_PATH_OVERRIDE_KEY: &str = "mssql.schema_search_path";
const EXPORT_SCHEMA_SQL_OVERRIDE_KEY: &str = "mssql.export_schema_sql";
const POISONED_CONNECTION_MESSAGE: &str = "mssql connection state was poisoned";

pub(crate) struct MssqlAdapter {
    backend: AdapterBackend,
    schema_search_path: Vec<String>,
    server_version: Version,
}

enum AdapterBackend {
    Live(Box<Mutex<LiveState>>),
    Override { export_schema_sql: String },
}

struct LiveState {
    runtime: Runtime,
    client: TdsClient,
}

#[derive(Debug)]
struct ExportColumn {
    name: String,
    data_type: String,
    max_length: i32,
    precision: i32,
    scale: i32,
    not_null: bool,
    identity: Option<IdentitySpec>,
}

#[derive(Debug)]
struct IdentitySpec {
    seed: i64,
    increment: i64,
    not_for_replication: bool,
}

#[derive(Debug)]
struct PrimaryKeySpec {
    name: String,
    clustered_kind: Option<&'static str>,
    columns: Vec<(String, bool)>,
}

pub(crate) fn connect(config: &ConnectionConfig) -> Result<Box<dyn DatabaseAdapter>> {
    if let Some(export_schema_sql) = config.extra.get(EXPORT_SCHEMA_SQL_OVERRIDE_KEY) {
        let raw_version = config
            .extra
            .get(SERVER_VERSION_OVERRIDE_KEY)
            .cloned()
            .unwrap_or_else(|| DEFAULT_MSSQL_VERSION.to_string());
        let server_version = parse_server_version(&raw_version)
            .ok_or_else(|| invalid_server_version_error(raw_version.as_str()))?;
        ensure_minimum_version(&server_version, raw_version.as_str())?;

        return Ok(Box::new(MssqlAdapter {
            backend: AdapterBackend::Override {
                export_schema_sql: normalize_export_schema_sql(export_schema_sql),
            },
            schema_search_path: schema_search_path_from_override(
                config.extra.get(SCHEMA_SEARCH_PATH_OVERRIDE_KEY),
            ),
            server_version,
        }));
    }

    if let Some(raw_version) = config.extra.get(SERVER_VERSION_OVERRIDE_KEY) {
        let parsed_version = parse_server_version(raw_version)
            .ok_or_else(|| invalid_server_version_error(raw_version.as_str()))?;
        ensure_minimum_version(&parsed_version, raw_version.as_str())?;
    }

    let mut live_state = connect_live_state(config)?;

    let raw_version = if let Some(raw_version) = config.extra.get(SERVER_VERSION_OVERRIDE_KEY) {
        raw_version.clone()
    } else {
        query_scalar_string(&mut live_state, export_queries::SHOW_SERVER_VERSION_QUERY)?
    };
    let server_version = parse_server_version(raw_version.as_str())
        .ok_or_else(|| invalid_server_version_error(raw_version.as_str()))?;
    ensure_minimum_version(&server_version, raw_version.as_str())?;

    let schema_search_path =
        if let Some(override_value) = config.extra.get(SCHEMA_SEARCH_PATH_OVERRIDE_KEY) {
            schema_search_path_from_override(Some(override_value))
        } else {
            let current_schema =
                query_scalar_string(&mut live_state, export_queries::CURRENT_SCHEMA_QUERY)?;
            schema_search_path_from_override(Some(&current_schema))
        };

    Ok(Box::new(MssqlAdapter {
        backend: AdapterBackend::Live(Box::new(Mutex::new(live_state))),
        schema_search_path,
        server_version,
    }))
}

impl DatabaseAdapter for MssqlAdapter {
    fn export_schema(&self) -> Result<String> {
        match &self.backend {
            AdapterBackend::Override { export_schema_sql } => Ok(export_schema_sql.clone()),
            AdapterBackend::Live(state) => {
                let mut state = lock_live_state(state, export_queries::TABLE_NAMES_QUERY)?;
                export_schema_live(&mut state)
            }
        }
    }

    fn execute(&self, sql: &str) -> Result<()> {
        match &self.backend {
            AdapterBackend::Override { .. } => Ok(()),
            AdapterBackend::Live(state) => {
                let mut state = lock_live_state(state, sql)?;
                execute_live_sql(&mut state, sql)
            }
        }
    }

    fn begin(&mut self) -> Result<Transaction<'_>> {
        self.execute(BEGIN_SQL)?;
        Ok(Transaction::new(self))
    }

    fn schema_search_path(&self) -> Vec<String> {
        self.schema_search_path.clone()
    }

    fn server_version(&self) -> Result<Version> {
        Ok(self.server_version.clone())
    }
}

fn connect_live_state(config: &ConnectionConfig) -> Result<LiveState> {
    let runtime = Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|source| execution_error(CONNECT_SQL, source))?;
    let tds_config = build_tiberius_config(config)?;

    let client = runtime.block_on(async {
        let tcp = TcpStream::connect(tds_config.get_addr())
            .await
            .map_err(|source| execution_error(CONNECT_SQL, source))?;
        tcp.set_nodelay(true)
            .map_err(|source| execution_error(CONNECT_SQL, source))?;

        Client::connect(tds_config, tcp.compat_write())
            .await
            .map_err(|source| execution_error(CONNECT_SQL, source))
    })?;

    Ok(LiveState { runtime, client })
}

fn build_tiberius_config(config: &ConnectionConfig) -> Result<Config> {
    let host = config
        .host
        .as_deref()
        .unwrap_or(DEFAULT_MSSQL_HOST)
        .to_string();
    let port = config.port.unwrap_or(DEFAULT_MSSQL_PORT);
    let user = config
        .user
        .clone()
        .ok_or_else(|| execution_error(CONNECT_SQL, io::Error::other("mssql user is required")))?;
    let password = config.password.clone().unwrap_or_default();

    let mut tds_config = Config::new();
    tds_config.host(host.as_str());
    tds_config.port(port);
    tds_config.database(config.database.clone());
    tds_config.authentication(AuthMethod::sql_server(user, password));
    tds_config.trust_cert();

    Ok(tds_config)
}

fn lock_live_state<'a>(
    state: &'a Mutex<LiveState>,
    sql: &str,
) -> Result<MutexGuard<'a, LiveState>> {
    state
        .lock()
        .map_err(|_| execution_error(sql, io::Error::other(POISONED_CONNECTION_MESSAGE)))
}

fn export_schema_live(state: &mut LiveState) -> Result<String> {
    let table_rows = query_rows(state, export_queries::TABLE_NAMES_QUERY)?;
    let mut statements = Vec::with_capacity(table_rows.len());

    for row in &table_rows {
        let schema_name = row
            .first()
            .map(String::as_str)
            .unwrap_or(DEFAULT_MSSQL_SCHEMA);
        let table_name = row.get(1).map(String::as_str).unwrap_or_default();
        if table_name.trim().is_empty() {
            continue;
        }
        statements.push(render_table_ddl(
            state,
            schema_name.trim(),
            table_name.trim(),
        )?);
    }

    Ok(statements.join("\n\n"))
}

fn render_table_ddl(state: &mut LiveState, schema_name: &str, table_name: &str) -> Result<String> {
    let object_id_literal = format!("{}.{}", quote_ident(schema_name), quote_ident(table_name));
    let columns_query = export_queries::COLUMN_DEFINITIONS_QUERY_TEMPLATE
        .replace("{object_id_literal}", object_id_literal.as_str());
    let primary_key_query = export_queries::PRIMARY_KEY_QUERY_TEMPLATE
        .replace("{object_id_literal}", object_id_literal.as_str());

    let column_rows = query_rows(state, columns_query.as_str())?;
    if column_rows.is_empty() {
        return Err(execution_error(
            columns_query.as_str(),
            io::Error::other("table export produced no columns"),
        ));
    }

    let columns = column_rows
        .iter()
        .map(|row| parse_export_column(row.as_slice()))
        .collect::<Result<Vec<_>>>()?;
    let primary_key = parse_primary_key_spec(&query_rows(state, primary_key_query.as_str())?);

    let mut definitions = columns.iter().map(render_export_column).collect::<Vec<_>>();

    if let Some(primary_key) = primary_key {
        definitions.push(render_primary_key(primary_key));
    }

    Ok(format!(
        "CREATE TABLE {} (\n    {}\n);",
        object_id_literal,
        definitions.join(",\n    ")
    ))
}

fn parse_export_column(row: &[String]) -> Result<ExportColumn> {
    let name = row.first().cloned().unwrap_or_default();
    let data_type = row.get(1).cloned().unwrap_or_default();
    let max_length = parse_i32_field(row.get(2));
    let precision = parse_i32_field(row.get(3));
    let scale = parse_i32_field(row.get(4));
    let not_null = row.get(5).is_some_and(|value| value.trim() == "0");
    let is_identity = row.get(6).is_some_and(|value| value.trim() == "1");

    if name.trim().is_empty() {
        return Err(execution_error(
            export_queries::COLUMN_DEFINITIONS_QUERY_TEMPLATE,
            io::Error::other("missing column name in export row"),
        ));
    }

    let identity = if is_identity {
        Some(IdentitySpec {
            seed: parse_i64_field(row.get(7)).unwrap_or(1),
            increment: parse_i64_field(row.get(8)).unwrap_or(1),
            not_for_replication: row.get(9).is_some_and(|value| value.trim() == "1"),
        })
    } else {
        None
    };

    Ok(ExportColumn {
        name,
        data_type,
        max_length,
        precision,
        scale,
        not_null,
        identity,
    })
}

fn parse_primary_key_spec(rows: &[Vec<String>]) -> Option<PrimaryKeySpec> {
    let first = rows.first()?;
    let name = first.first()?.trim().to_string();
    if name.is_empty() {
        return None;
    }

    let type_desc = first.get(1).map(|value| value.to_ascii_uppercase());
    let clustered_kind = match type_desc.as_deref() {
        Some(value) if value.contains("NONCLUSTERED") => Some("NONCLUSTERED"),
        Some(value) if value.contains("CLUSTERED") => Some("CLUSTERED"),
        _ => None,
    };

    let columns = rows
        .iter()
        .filter_map(|row| {
            let name = row.get(2)?.trim().to_string();
            if name.is_empty() {
                return None;
            }
            let is_descending = row.get(3).is_some_and(|value| value.trim() == "1");
            Some((name, is_descending))
        })
        .collect::<Vec<_>>();

    if columns.is_empty() {
        return None;
    }

    Some(PrimaryKeySpec {
        name,
        clustered_kind,
        columns,
    })
}

fn render_export_column(column: &ExportColumn) -> String {
    let mut sql = format!(
        "{} {}",
        quote_ident(column.name.as_str()),
        render_export_data_type(column)
    );

    if column.not_null {
        sql.push_str(" NOT NULL");
    }

    if let Some(identity) = &column.identity {
        sql.push_str(" IDENTITY(");
        sql.push_str(identity.seed.to_string().as_str());
        sql.push(',');
        sql.push_str(identity.increment.to_string().as_str());
        sql.push(')');
        if identity.not_for_replication {
            sql.push_str(" NOT FOR REPLICATION");
        }
    }

    sql
}

fn render_primary_key(primary_key: PrimaryKeySpec) -> String {
    let mut sql = format!(
        "CONSTRAINT {} PRIMARY KEY",
        quote_ident(primary_key.name.as_str())
    );
    if let Some(clustered_kind) = primary_key.clustered_kind {
        sql.push(' ');
        sql.push_str(clustered_kind);
    }

    let columns = primary_key
        .columns
        .iter()
        .map(|(name, is_descending)| {
            format!(
                "{} {}",
                quote_ident(name.as_str()),
                if *is_descending { "DESC" } else { "ASC" }
            )
        })
        .collect::<Vec<_>>()
        .join(", ");

    sql.push_str(" (");
    sql.push_str(columns.as_str());
    sql.push(')');
    sql
}

fn render_export_data_type(column: &ExportColumn) -> String {
    let data_type = column.data_type.trim().to_ascii_uppercase();
    match data_type.as_str() {
        "NVARCHAR" | "NCHAR" => {
            if column.max_length == -1 {
                format!("{data_type}(MAX)")
            } else {
                let length = (column.max_length / 2).max(1);
                format!("{data_type}({length})")
            }
        }
        "VARCHAR" | "CHAR" | "VARBINARY" | "BINARY" => {
            if column.max_length == -1 {
                format!("{data_type}(MAX)")
            } else {
                let length = column.max_length.max(1);
                format!("{data_type}({length})")
            }
        }
        "DECIMAL" | "NUMERIC" => {
            if column.precision > 0 {
                format!("{data_type}({}, {})", column.precision, column.scale.max(0))
            } else {
                data_type
            }
        }
        "DATETIME2" | "TIME" => {
            if column.scale > 0 {
                format!("{data_type}({})", column.scale)
            } else {
                data_type
            }
        }
        _ => data_type,
    }
}

fn execute_live_sql(state: &mut LiveState, sql: &str) -> Result<()> {
    let LiveState { runtime, client } = state;

    runtime.block_on(async {
        let mut stream = client
            .simple_query(sql)
            .await
            .map_err(|source| execution_error(sql, source))?;
        while stream
            .try_next()
            .await
            .map_err(|source| execution_error(sql, source))?
            .is_some()
        {}
        Ok(())
    })
}

fn query_scalar_string(state: &mut LiveState, sql: &str) -> Result<String> {
    let rows = query_rows(state, sql)?;

    rows.into_iter()
        .next()
        .and_then(|columns| columns.into_iter().next())
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .ok_or_else(|| execution_error(sql, io::Error::other("query returned no rows")))
}

fn query_rows(state: &mut LiveState, sql: &str) -> Result<Vec<Vec<String>>> {
    let LiveState { runtime, client } = state;

    runtime.block_on(async {
        let mut stream = client
            .simple_query(sql)
            .await
            .map_err(|source| execution_error(sql, source))?;
        let mut rows = Vec::new();

        while let Some(item) = stream
            .try_next()
            .await
            .map_err(|source| execution_error(sql, source))?
        {
            if let QueryItem::Row(row) = item {
                let mut values = Vec::with_capacity(row.columns().len());
                for index in 0..row.columns().len() {
                    values.push(
                        row.get::<&str, usize>(index)
                            .unwrap_or_default()
                            .to_string(),
                    );
                }
                rows.push(values);
            }
        }

        Ok(rows)
    })
}

pub(crate) fn parse_server_version(raw: &str) -> Option<Version> {
    let mut parts = raw.split_whitespace().next()?.split('.');
    let major = parse_u16_component(parts.next()?)?;
    let minor = parts.next().and_then(parse_u16_component).unwrap_or(0);
    let patch = parts.next().and_then(parse_u16_component).unwrap_or(0);

    Some(Version {
        major,
        minor,
        patch,
    })
}

fn parse_u16_component(raw: &str) -> Option<u16> {
    let digits = raw
        .chars()
        .take_while(|ch| ch.is_ascii_digit())
        .collect::<String>();
    if digits.is_empty() {
        return None;
    }
    digits.parse::<u16>().ok()
}

fn parse_i32_field(raw: Option<&String>) -> i32 {
    raw.and_then(|value| value.trim().parse::<i32>().ok())
        .unwrap_or_default()
}

fn parse_i64_field(raw: Option<&String>) -> Option<i64> {
    raw.and_then(|value| value.trim().parse::<i64>().ok())
}

fn ensure_minimum_version(version: &Version, raw_version: &str) -> Result<()> {
    let supported = if version.major >= YEAR_VERSION_THRESHOLD {
        version.major >= MINIMUM_MSSQL_YEAR_VERSION
    } else {
        version.major >= MINIMUM_MSSQL_PRODUCT_MAJOR_VERSION
    };

    if supported {
        return Ok(());
    }

    Err(execution_error(
        export_queries::SHOW_SERVER_VERSION_QUERY,
        io::Error::other(format!(
            "mssql server version `{raw_version}` is not supported; requires SQL Server 2019+"
        )),
    ))
}

fn invalid_server_version_error(raw_version: &str) -> stateql_core::Error {
    execution_error(
        export_queries::SHOW_SERVER_VERSION_QUERY,
        io::Error::other(format!(
            "failed to parse mssql server version string: `{raw_version}`"
        )),
    )
}

fn schema_search_path_from_override(override_value: Option<&String>) -> Vec<String> {
    let mut values = override_value
        .map(|value| parse_schema_search_path(value.as_str()))
        .unwrap_or_default();
    if values.is_empty() {
        values.push(DEFAULT_MSSQL_SCHEMA.to_string());
    }
    values
}

fn parse_schema_search_path(raw: &str) -> Vec<String> {
    raw.split(',')
        .map(str::trim)
        .filter(|entry| !entry.is_empty())
        .map(ToString::to_string)
        .collect()
}

fn normalize_export_schema_sql(raw: &str) -> String {
    raw.trim().to_string()
}

fn quote_ident(identifier: &str) -> String {
    format!("[{}]", identifier.replace(']', "]]"))
}

fn execution_error<E>(sql: &str, source: E) -> stateql_core::Error
where
    E: StdError + Send + Sync + 'static,
{
    ExecutionError::statement_failed(0, sql, 0, None, None, source).into()
}
