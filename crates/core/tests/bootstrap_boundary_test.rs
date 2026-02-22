use std::{fs, path::PathBuf};

use stateql_core::{DiffError, Error, Result};

fn read_source(file_name: &str) -> String {
    let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    path.push("src");
    path.push(file_name);

    fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("failed to read source file {}: {error}", path.display()))
}

#[test]
fn bootstrap_symbols_are_removed() {
    let lib_source = read_source("lib.rs");
    let diff_source = read_source("diff.rs");

    let plan_symbol = ["plan", "_", "diff"].concat();
    let create_symbol = ["Create", "Object"].concat();
    let drop_symbol = ["Drop", "Object"].concat();
    let smoke_symbol = ["smoke", "_", "parse", "_", "diff", "_", "render"].concat();

    assert!(
        !lib_source.contains(&plan_symbol),
        "bootstrap symbol must be removed: {plan_symbol}",
    );
    assert!(
        !lib_source.contains(&smoke_symbol),
        "bootstrap symbol must be removed: {smoke_symbol}",
    );
    assert!(
        !diff_source.contains(&create_symbol),
        "bootstrap symbol must be removed: {create_symbol}",
    );
    assert!(
        !diff_source.contains(&drop_symbol),
        "bootstrap symbol must be removed: {drop_symbol}",
    );
}

#[test]
fn top_level_error_and_result_are_public() {
    let result: Result<()> = Ok(());
    assert!(result.is_ok());

    let error: Error = DiffError::ObjectComparison {
        target: "users".to_string(),
        operation: "missing owner".to_string(),
    }
    .into();
    assert!(matches!(error, Error::Diff(_)));
}
