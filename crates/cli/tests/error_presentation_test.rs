use std::{
    io::Write,
    process::{Command, Stdio},
};

use tempfile::tempdir;

fn run_stateql_with_stdin(args: &[&str], stdin_sql: &str) -> std::process::Output {
    let mut child = Command::new(env!("CARGO_BIN_EXE_stateql"))
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap_or_else(|error| panic!("failed to run stateql with stdin: {error}"));

    let mut stdin = child
        .stdin
        .take()
        .unwrap_or_else(|| panic!("failed to capture child stdin"));
    stdin
        .write_all(stdin_sql.as_bytes())
        .unwrap_or_else(|error| panic!("failed to write stdin payload: {error}"));
    drop(stdin);

    child
        .wait_with_output()
        .unwrap_or_else(|error| panic!("failed to wait for stateql: {error}"))
}

#[cfg(feature = "sqlite")]
#[test]
fn runtime_parse_error_keeps_typed_category_with_cli_context() {
    let tempdir = tempdir().unwrap_or_else(|error| panic!("failed to create tempdir: {error}"));
    let db_path = tempdir.path().join("error-presentation.db");
    let db_path = db_path.to_string_lossy().into_owned();

    let output = run_stateql_with_stdin(&["sqlite", db_path.as_str()], "SELECT 1;");

    assert_eq!(output.status.code(), Some(1));

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("[parse]"),
        "stderr must preserve typed parse category, got: {stderr}",
    );
    assert!(
        stderr.contains("while running orchestrator"),
        "stderr must include CLI context from anyhow::Context, got: {stderr}",
    );
    assert!(
        stderr.contains("parse statement[0] failed"),
        "stderr must retain typed parse details, got: {stderr}",
    );
}
