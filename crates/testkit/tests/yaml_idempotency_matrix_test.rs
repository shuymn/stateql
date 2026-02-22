use std::path::PathBuf;

use stateql_core::Dialect;
use stateql_dialect_mssql::MssqlDialect;
use stateql_dialect_mysql::MysqlDialect;
use stateql_dialect_postgres::PostgresDialect;
use stateql_dialect_sqlite::SqliteDialect;
use stateql_testkit::{TestResult, load_test_cases_from_dir, run_offline_test};

#[test]
fn idempotency_yaml_cases_pass_for_all_dialects() {
    run_idempotency_matrix("postgres", &PostgresDialect);
    run_idempotency_matrix("sqlite", &SqliteDialect);
    run_idempotency_matrix("mysql", &MysqlDialect);
    run_idempotency_matrix("mssql", &MssqlDialect);
}

fn run_idempotency_matrix(dialect_name: &str, dialect: &dyn Dialect) {
    let case_files =
        load_test_cases_from_dir(idempotency_root(dialect_name)).unwrap_or_else(|error| {
            panic!("failed to load idempotency case files for dialect '{dialect_name}': {error}")
        });

    assert!(
        !case_files.is_empty(),
        "idempotency directory for '{dialect_name}' must not be empty"
    );

    let mut case_count = 0_usize;
    for case_file in case_files {
        assert!(
            !case_file.cases.is_empty(),
            "idempotency file '{}' for dialect '{}' must contain at least one testcase",
            case_file.path.display(),
            dialect_name
        );

        for (case_name, case) in case_file.cases {
            case_count += 1;
            match run_offline_test(dialect, &case) {
                TestResult::Passed => {}
                TestResult::Skipped(reason) => panic!(
                    "dialect '{}' testcase '{}::{}' unexpectedly skipped: {}",
                    dialect_name, case_file.file_name, case_name, reason
                ),
                TestResult::Failed(reason) => panic!(
                    "dialect '{}' testcase '{}::{}' failed: {}",
                    dialect_name, case_file.file_name, case_name, reason
                ),
            }
        }
    }

    assert!(
        case_count >= 25,
        "dialect '{}' must have at least 25 ported idempotency cases, found {}",
        dialect_name,
        case_count
    );
}

fn idempotency_root(dialect_name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests")
        .join(dialect_name)
        .join("idempotency")
}
