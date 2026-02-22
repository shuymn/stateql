use std::{
    fs,
    io::Write,
    process::{Command, Stdio},
};

use tempfile::tempdir;

fn run_stateql(args: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_stateql"))
        .args(args)
        .output()
        .unwrap_or_else(|error| panic!("failed to run stateql: {error}"))
}

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

#[test]
fn rejects_apply_and_export_even_with_enable_drop() {
    let tempdir = tempdir().unwrap_or_else(|error| panic!("failed to create tempdir: {error}"));
    let db_path = tempdir.path().join("conflict.db");
    let db_path = db_path.to_string_lossy().into_owned();

    let output = run_stateql(&[
        "sqlite",
        db_path.as_str(),
        "--apply",
        "--export",
        "--enable-drop",
    ]);

    assert_eq!(output.status.code(), Some(2));

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("--apply"));
    assert!(stderr.contains("--export"));
}

#[test]
fn defaults_to_dry_run_when_file_input_is_provided() {
    let tempdir = tempdir().unwrap_or_else(|error| panic!("failed to create tempdir: {error}"));
    let db_path = tempdir.path().join("dry-run-file.db");
    let db_path = db_path.to_string_lossy().into_owned();
    let schema_path = tempdir.path().join("schema.sql");

    fs::write(
        &schema_path,
        "CREATE TABLE users (id INTEGER PRIMARY KEY, name TEXT);",
    )
    .unwrap_or_else(|error| panic!("failed to write schema.sql: {error}"));

    let schema_path = schema_path.to_string_lossy().into_owned();

    let output = run_stateql(&[
        "sqlite",
        "--file",
        schema_path.as_str(),
        db_path.as_str(),
        "--enable-drop",
    ]);

    assert_eq!(output.status.code(), Some(0));

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("CREATE TABLE users"));
}

#[test]
fn defaults_to_dry_run_when_stdin_input_is_provided() {
    let tempdir = tempdir().unwrap_or_else(|error| panic!("failed to create tempdir: {error}"));
    let db_path = tempdir.path().join("dry-run-stdin.db");
    let db_path = db_path.to_string_lossy().into_owned();

    let output = run_stateql_with_stdin(
        &["sqlite", db_path.as_str()],
        "CREATE TABLE projects (id INTEGER PRIMARY KEY);",
    );

    assert_eq!(output.status.code(), Some(0));

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("CREATE TABLE projects"));
}
