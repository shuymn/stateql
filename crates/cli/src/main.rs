use std::{
    collections::BTreeMap,
    error::Error as StdError,
    fmt, fs,
    io::{self, IsTerminal, Read},
    path::{Path, PathBuf},
};

use clap::{Args, Parser, Subcommand};
use stateql_core::{
    ConnectionConfig, Dialect, Mode, Orchestrator, OrchestratorOptions, OrchestratorOutput,
};
#[cfg(feature = "mssql")]
use stateql_dialect_mssql::MssqlDialect;
#[cfg(feature = "mysql")]
use stateql_dialect_mysql::MysqlDialect;
#[cfg(feature = "postgres")]
use stateql_dialect_postgres::PostgresDialect;
#[cfg(feature = "sqlite")]
use stateql_dialect_sqlite::SqliteDialect;

const RUNTIME_ERROR_EXIT_CODE: i32 = 1;
#[cfg(feature = "postgres")]
const POSTGRES_SSLMODE_KEY: &str = "postgres.sslmode";

#[derive(Parser, Debug)]
#[command(name = "stateql")]
struct Cli {
    #[command(subcommand)]
    dialect: DialectCommand,
}

#[derive(Subcommand, Debug)]
enum DialectCommand {
    #[cfg(feature = "mysql")]
    Mysql(MysqlArgs),
    #[cfg(feature = "postgres")]
    Postgres(PostgresArgs),
    #[cfg(feature = "sqlite")]
    Sqlite(SqliteArgs),
    #[cfg(feature = "mssql")]
    Mssql(MssqlArgs),
    #[cfg(not(any(
        feature = "mysql",
        feature = "postgres",
        feature = "sqlite",
        feature = "mssql"
    )))]
    #[command(hide = true)]
    NoDialectsEnabled,
}

#[derive(Args, Debug, Clone)]
struct ModeArgs {
    #[arg(long, conflicts_with_all = ["dry_run", "export"])]
    apply: bool,
    #[arg(long = "dry-run", conflicts_with_all = ["apply", "export"])]
    dry_run: bool,
    #[arg(long, conflicts_with_all = ["apply", "dry_run"])]
    export: bool,
    #[arg(long)]
    file: Option<PathBuf>,
    #[arg(long)]
    enable_drop: bool,
}

impl ModeArgs {
    fn explicit_mode(&self) -> Option<Mode> {
        if self.apply {
            Some(Mode::Apply)
        } else if self.dry_run {
            Some(Mode::DryRun)
        } else if self.export {
            Some(Mode::Export)
        } else {
            None
        }
    }
}

#[derive(Args, Debug)]
struct TcpConnectionArgs {
    #[arg(long)]
    host: Option<String>,
    #[arg(long)]
    port: Option<u16>,
    #[arg(long)]
    user: Option<String>,
    #[arg(long)]
    password: Option<String>,
}

#[derive(Args, Debug)]
struct MysqlArgs {
    #[command(flatten)]
    mode: ModeArgs,
    #[command(flatten)]
    connection: TcpConnectionArgs,
    #[arg(long)]
    socket: Option<String>,
    #[arg(value_name = "DATABASE")]
    database: String,
}

#[derive(Args, Debug)]
struct PostgresArgs {
    #[command(flatten)]
    mode: ModeArgs,
    #[command(flatten)]
    connection: TcpConnectionArgs,
    #[arg(long)]
    sslmode: Option<String>,
    #[arg(value_name = "DATABASE")]
    database: String,
}

#[derive(Args, Debug)]
struct SqliteArgs {
    #[command(flatten)]
    mode: ModeArgs,
    #[arg(value_name = "DATABASE")]
    database: String,
}

#[derive(Args, Debug)]
struct MssqlArgs {
    #[command(flatten)]
    mode: ModeArgs,
    #[command(flatten)]
    connection: TcpConnectionArgs,
    #[arg(value_name = "DATABASE")]
    database: String,
}

type CliResult<T> = std::result::Result<T, CliError>;

#[derive(Debug)]
enum CliError {
    MissingDesiredSchemaInput,
    ReadFile {
        path: PathBuf,
        source: io::Error,
    },
    ReadStdin(io::Error),
    Core(stateql_core::Error),
    #[cfg(not(any(
        feature = "mysql",
        feature = "postgres",
        feature = "sqlite",
        feature = "mssql"
    )))]
    NoDialectsEnabled,
}

impl fmt::Display for CliError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingDesiredSchemaInput => {
                write!(
                    f,
                    "missing desired schema SQL: pass --file <PATH> or pipe SQL via stdin"
                )
            }
            Self::ReadFile { path, source } => {
                write!(
                    f,
                    "failed to read desired schema file `{}`: {source}",
                    path.display()
                )
            }
            Self::ReadStdin(source) => {
                write!(f, "failed to read desired schema from stdin: {source}")
            }
            Self::Core(source) => write!(f, "{source}"),
            #[cfg(not(any(
                feature = "mysql",
                feature = "postgres",
                feature = "sqlite",
                feature = "mssql"
            )))]
            Self::NoDialectsEnabled => write!(
                f,
                "no dialect features are enabled for this build; enable at least one of mysql/postgres/sqlite/mssql"
            ),
        }
    }
}

impl StdError for CliError {
    fn source(&self) -> Option<&(dyn StdError + 'static)> {
        match self {
            Self::ReadFile { source, .. } => Some(source),
            Self::ReadStdin(source) => Some(source),
            Self::Core(source) => Some(source),
            Self::MissingDesiredSchemaInput => None,
            #[cfg(not(any(
                feature = "mysql",
                feature = "postgres",
                feature = "sqlite",
                feature = "mssql"
            )))]
            Self::NoDialectsEnabled => None,
        }
    }
}

impl From<stateql_core::Error> for CliError {
    fn from(value: stateql_core::Error) -> Self {
        Self::Core(value)
    }
}

fn main() {
    let cli = match Cli::try_parse() {
        Ok(cli) => cli,
        Err(error) => {
            let exit_code = error.exit_code();
            let _ = error.print();
            std::process::exit(exit_code);
        }
    };

    if let Err(error) = run(cli) {
        eprintln!("{error}");
        std::process::exit(RUNTIME_ERROR_EXIT_CODE);
    }
}

fn run(cli: Cli) -> CliResult<()> {
    match cli.dialect {
        #[cfg(feature = "mysql")]
        DialectCommand::Mysql(args) => {
            let MysqlArgs {
                mode,
                connection,
                socket,
                database,
            } = args;
            let config = connection_config(connection, database, socket, BTreeMap::new());
            run_with_dialect(&MysqlDialect, config, mode)
        }
        #[cfg(feature = "postgres")]
        DialectCommand::Postgres(args) => {
            let PostgresArgs {
                mode,
                connection,
                sslmode,
                database,
            } = args;
            let mut extra = BTreeMap::new();
            if let Some(sslmode) = sslmode {
                extra.insert(POSTGRES_SSLMODE_KEY.to_string(), sslmode);
            }
            let config = connection_config(connection, database, None, extra);
            run_with_dialect(&PostgresDialect, config, mode)
        }
        #[cfg(feature = "sqlite")]
        DialectCommand::Sqlite(args) => {
            let SqliteArgs { mode, database } = args;
            let config = ConnectionConfig {
                host: None,
                port: None,
                user: None,
                password: None,
                database,
                socket: None,
                extra: BTreeMap::new(),
            };
            run_with_dialect(&SqliteDialect, config, mode)
        }
        #[cfg(feature = "mssql")]
        DialectCommand::Mssql(args) => {
            let MssqlArgs {
                mode,
                connection,
                database,
            } = args;
            let config = connection_config(connection, database, None, BTreeMap::new());
            run_with_dialect(&MssqlDialect, config, mode)
        }
        #[cfg(not(any(
            feature = "mysql",
            feature = "postgres",
            feature = "sqlite",
            feature = "mssql"
        )))]
        DialectCommand::NoDialectsEnabled => Err(CliError::NoDialectsEnabled),
    }
}

fn connection_config(
    connection: TcpConnectionArgs,
    database: String,
    socket: Option<String>,
    extra: BTreeMap<String, String>,
) -> ConnectionConfig {
    ConnectionConfig {
        host: connection.host,
        port: connection.port,
        user: connection.user,
        password: connection.password,
        database,
        socket,
        extra,
    }
}

fn run_with_dialect(
    dialect: &dyn Dialect,
    connection_config: ConnectionConfig,
    mode_args: ModeArgs,
) -> CliResult<()> {
    let explicit_mode = mode_args.explicit_mode();
    let desired_sql = if explicit_mode == Some(Mode::Export) {
        None
    } else {
        read_desired_sql(mode_args.file.as_deref())?
    };

    let mode = match explicit_mode {
        Some(mode) => mode,
        None if desired_sql.is_some() => Mode::DryRun,
        None => return Err(CliError::MissingDesiredSchemaInput),
    };

    if matches!(mode, Mode::Apply | Mode::DryRun) && desired_sql.is_none() {
        return Err(CliError::MissingDesiredSchemaInput);
    }

    let orchestrator = Orchestrator::new(dialect);
    let output = orchestrator.run(
        &connection_config,
        desired_sql.as_deref().unwrap_or(""),
        OrchestratorOptions {
            mode,
            enable_drop: mode_args.enable_drop,
        },
    )?;

    match output {
        OrchestratorOutput::Applied => {}
        OrchestratorOutput::DryRunSql(sql) | OrchestratorOutput::ExportSql(sql) => {
            print!("{sql}");
        }
    }

    Ok(())
}

fn read_desired_sql(path: Option<&Path>) -> CliResult<Option<String>> {
    if let Some(path) = path {
        return fs::read_to_string(path)
            .map(Some)
            .map_err(|source| CliError::ReadFile {
                path: path.to_path_buf(),
                source,
            });
    }

    if io::stdin().is_terminal() {
        return Ok(None);
    }

    let mut input = String::new();
    io::stdin()
        .read_to_string(&mut input)
        .map_err(CliError::ReadStdin)?;
    if input.is_empty() {
        Ok(None)
    } else {
        Ok(Some(input))
    }
}
