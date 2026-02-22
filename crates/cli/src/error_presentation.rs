use std::{io, path::PathBuf};

use anyhow::Context;
use miette::Report;

const ORCHESTRATOR_CONTEXT: &str = "while running orchestrator";
const FILE_READ_CONTEXT: &str = "while reading desired schema file";
const STDIN_READ_CONTEXT: &str = "while reading desired schema from stdin";

pub(crate) type CliResult<T> = std::result::Result<T, CliError>;

#[derive(Debug)]
pub(crate) enum CliError {
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

impl From<stateql_core::Error> for CliError {
    fn from(value: stateql_core::Error) -> Self {
        Self::Core(value)
    }
}

pub(crate) fn render_runtime_error(error: CliError) -> String {
    match error {
        CliError::MissingDesiredSchemaInput => {
            format!("[usage] {}", missing_desired_schema_message())
        }
        CliError::ReadFile { path, source } => {
            let context = format!("{FILE_READ_CONTEXT} `{}`", path.display());
            let report = report_with_context(source, context);
            format!("[io] {report}")
        }
        CliError::ReadStdin(source) => {
            let report = report_with_context(source, STDIN_READ_CONTEXT);
            format!("[io] {report}")
        }
        CliError::Core(source) => {
            let category = core_category(&source);
            let report = report_with_context(source, ORCHESTRATOR_CONTEXT);
            format!("[{category}] {report}")
        }
        #[cfg(not(any(
            feature = "mysql",
            feature = "postgres",
            feature = "sqlite",
            feature = "mssql"
        )))]
        CliError::NoDialectsEnabled => format!("[config] {}", no_dialects_enabled_message()),
    }
}

fn report_with_context<E, C>(source: E, context: C) -> Report
where
    E: std::error::Error + Send + Sync + 'static,
    C: Into<String>,
{
    let context = context.into();
    let anyhow_error = std::result::Result::<(), E>::Err(source)
        .context(context)
        .expect_err("context wrapping must produce an error");
    miette::miette!("{anyhow_error:#}")
}

fn core_category(error: &stateql_core::Error) -> &'static str {
    match error {
        stateql_core::Error::Parse(_) => "parse",
        stateql_core::Error::Diff(_) => "diff",
        stateql_core::Error::Generate(_) => "generate",
        stateql_core::Error::Execute(_) => "execute",
    }
}

fn missing_desired_schema_message() -> &'static str {
    "missing desired schema SQL: pass --file <PATH> or pipe SQL via stdin"
}

#[cfg(not(any(
    feature = "mysql",
    feature = "postgres",
    feature = "sqlite",
    feature = "mssql"
)))]
fn no_dialects_enabled_message() -> &'static str {
    "no dialect features are enabled for this build; enable at least one of mysql/postgres/sqlite/mssql"
}
