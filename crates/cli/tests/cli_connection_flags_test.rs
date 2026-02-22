use std::process::Command;

fn run_stateql(args: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_stateql"))
        .args(args)
        .output()
        .unwrap_or_else(|error| panic!("failed to run stateql: {error}"))
}

#[cfg(feature = "mysql")]
#[test]
fn mysql_help_lists_common_and_mysql_specific_connection_flags() {
    let output = run_stateql(&["mysql", "--help"]);

    assert_eq!(output.status.code(), Some(0));

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("--host"));
    assert!(stdout.contains("--port"));
    assert!(stdout.contains("--user"));
    assert!(stdout.contains("--password"));
    assert!(stdout.contains("--socket"));
    assert!(stdout.contains("<DATABASE>"));
}

#[cfg(feature = "postgres")]
#[test]
fn postgres_help_lists_common_and_postgres_specific_connection_flags() {
    let output = run_stateql(&["postgres", "--help"]);

    assert_eq!(output.status.code(), Some(0));

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("--host"));
    assert!(stdout.contains("--port"));
    assert!(stdout.contains("--user"));
    assert!(stdout.contains("--password"));
    assert!(stdout.contains("--sslmode"));
    assert!(stdout.contains("<DATABASE>"));
}

#[cfg(feature = "sqlite")]
#[test]
fn sqlite_help_uses_database_path_and_excludes_network_flags() {
    let output = run_stateql(&["sqlite", "--help"]);

    assert_eq!(output.status.code(), Some(0));

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("<DATABASE>"));
    assert!(!stdout.contains("--host"));
    assert!(!stdout.contains("--port"));
    assert!(!stdout.contains("--user"));
    assert!(!stdout.contains("--password"));
    assert!(!stdout.contains("--socket"));
    assert!(!stdout.contains("--sslmode"));
}
