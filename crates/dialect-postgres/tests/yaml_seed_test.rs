use std::path::PathBuf;

use stateql_dialect_postgres::PostgresDialect;
use stateql_testkit::{TestResult, load_test_cases_from_path, run_offline_test};

#[test]
fn postgres_yaml_seed_cases_pass_offline_runner() {
    let dialect = PostgresDialect;

    run_seed_case_file(&dialect, "0001-basic-create.yml");
    run_seed_case_file(&dialect, "0002-add-index.yml");
}

fn run_seed_case_file(dialect: &PostgresDialect, file_name: &str) {
    let path = seed_cases_root().join(file_name);
    let cases = load_test_cases_from_path(&path)
        .unwrap_or_else(|error| panic!("failed to load seed case '{}': {error}", path.display()));

    assert!(
        !cases.is_empty(),
        "seed case file '{}' must define at least one testcase",
        path.display()
    );

    for (case_name, case) in cases {
        match run_offline_test(dialect, &case) {
            TestResult::Passed => {}
            TestResult::Skipped(reason) => {
                panic!("seed testcase '{case_name}' unexpectedly skipped: {reason}")
            }
            TestResult::Failed(reason) => {
                panic!("seed testcase '{case_name}' failed: {reason}")
            }
        }
    }
}

fn seed_cases_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/postgres/idempotency")
}
