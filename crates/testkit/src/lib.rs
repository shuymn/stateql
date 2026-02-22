use stateql_core::{SchemaObject, Table};

mod yaml_runner;

pub use yaml_runner::{
    DialectIdempotencyManifest, IdempotencyManifest, IdempotencyManifestEntry, ManifestCoverage,
    ManifestStatus, TestCase, TestCaseFile, TestResult, idempotency_manifest_coverage,
    load_idempotency_manifest_from_path, load_test_cases_from_dir, load_test_cases_from_path,
    load_test_cases_from_str, matches_flavor, run_offline_test, run_online_test,
    validate_idempotency_manifest_entries,
};

pub fn single_table_fixture(name: &str) -> Vec<SchemaObject> {
    vec![SchemaObject::Table(Table::named(name))]
}
