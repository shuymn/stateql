use std::process::Command;

fn run_stateql(args: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_stateql"))
        .args(args)
        .output()
        .unwrap_or_else(|err| panic!("failed to run stateql: {err}"))
}

#[test]
fn usage_lists_default_enabled_dialects_only() {
    let output = run_stateql(&[]);

    assert_eq!(output.status.code(), Some(2));

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("Usage: stateql <COMMAND>"));
    assert!(stderr.contains("mysql"));
    assert!(stderr.contains("postgres"));
    assert!(stderr.contains("sqlite"));
    assert!(!stderr.contains("\nmssql"));
}

#[test]
fn rejects_disabled_mssql_subcommand_by_default() {
    let output = run_stateql(&["mssql"]);

    assert_eq!(output.status.code(), Some(2));

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("unrecognized subcommand 'mssql'"));
    assert!(stderr.contains("Usage: stateql <COMMAND>"));
}
