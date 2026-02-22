use std::path::PathBuf;

use stateql_core::Dialect;
use stateql_dialect_mssql::MssqlDialect;
use stateql_dialect_mysql::MysqlDialect;
use stateql_dialect_postgres::PostgresDialect;
use stateql_dialect_sqlite::SqliteDialect;
use stateql_testkit::{TestResult, load_test_cases_from_dir, run_offline_test};

const GROUPS: [&str; 4] = ["tables", "indexes", "constraints", "views"];

#[test]
fn assertion_yaml_cases_pass_for_all_dialects_and_groups() {
    run_assertion_matrix("postgres", &PostgresDialect);
    run_assertion_matrix("sqlite", &SqliteDialect);
    run_assertion_matrix("mysql", &MysqlDialect);
    run_assertion_matrix("mssql", &MssqlDialect);
}

fn run_assertion_matrix(dialect_name: &str, dialect: &dyn Dialect) {
    for group_name in GROUPS {
        let case_files = load_test_cases_from_dir(assertion_root(dialect_name, group_name))
            .unwrap_or_else(|error| {
                panic!(
                    "failed to load assertion case files for dialect '{dialect_name}' group '{group_name}': {error}"
                )
            });

        assert!(
            !case_files.is_empty(),
            "assertion directory for dialect '{dialect_name}' group '{group_name}' must not be empty"
        );

        let mut case_count = 0_usize;
        for case_file in case_files {
            assert!(
                !case_file.cases.is_empty(),
                "assertion file '{}::{}' must contain at least one testcase",
                dialect_name,
                case_file.path.display()
            );

            for (case_name, case) in case_file.cases {
                case_count += 1;
                match run_offline_test(dialect, &case) {
                    TestResult::Passed => {}
                    TestResult::Skipped(reason) => panic!(
                        "dialect '{}' group '{}' testcase '{}::{}' unexpectedly skipped: {}",
                        dialect_name, group_name, case_file.file_name, case_name, reason
                    ),
                    TestResult::Failed(reason) => panic!(
                        "dialect '{}' group '{}' testcase '{}::{}' failed: {}",
                        dialect_name, group_name, case_file.file_name, case_name, reason
                    ),
                }
            }
        }

        assert!(
            case_count >= 5,
            "dialect '{}' group '{}' must have at least 5 ported assertion cases, found {}",
            dialect_name,
            group_name,
            case_count
        );
    }
}

fn assertion_root(dialect_name: &str, group_name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests")
        .join(dialect_name)
        .join("assertions")
        .join(group_name)
}
