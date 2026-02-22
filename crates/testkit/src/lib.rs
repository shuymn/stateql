use stateql_core::{SchemaObject, Table};

mod yaml_runner;

pub use yaml_runner::{
    AssertionManifest, AssertionManifestGroup, DialectAssertionManifest,
    DialectIdempotencyManifest, IdempotencyManifest, IdempotencyManifestEntry, ManifestCoverage,
    ManifestStatus, TestCase, TestCaseFile, TestResult, assertion_manifest_coverage,
    idempotency_manifest_coverage, load_assertion_manifest_from_path,
    load_idempotency_manifest_from_path, load_test_cases_from_dir, load_test_cases_from_path,
    load_test_cases_from_str, manifest_ported_case_references, matches_flavor,
    normalize_sql_for_quote_aware_comparison, run_offline_test, run_online_test,
    sql_matches_quote_aware, validate_assertion_manifest_entries,
    validate_idempotency_manifest_entries, yaml_case_references,
};

pub fn single_table_fixture(name: &str) -> Vec<SchemaObject> {
    vec![SchemaObject::Table(Table::named(name))]
}
