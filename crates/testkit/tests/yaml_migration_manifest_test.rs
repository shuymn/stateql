use std::path::PathBuf;

use stateql_testkit::{
    IdempotencyManifestEntry, idempotency_manifest_coverage, load_idempotency_manifest_from_path,
    load_test_cases_from_dir, manifest_ported_case_references,
    validate_idempotency_manifest_entries, yaml_case_references,
};

const DIALECTS: [&str; 4] = ["postgres", "sqlite", "mysql", "mssql"];

#[test]
fn idempotency_manifest_meets_count_and_coverage_gates() {
    let manifest = load_idempotency_manifest_from_path(idempotency_manifest_path())
        .unwrap_or_else(|error| panic!("failed to load idempotency manifest: {error}"));

    for dialect_name in DIALECTS {
        let dialect_manifest = manifest
            .dialects
            .get(dialect_name)
            .unwrap_or_else(|| panic!("manifest is missing dialect '{dialect_name}'"));

        validate_idempotency_manifest_entries(&dialect_manifest.entries).unwrap_or_else(|error| {
            panic!(
                "manifest entry validation failed for dialect '{}': {}",
                dialect_name, error
            )
        });

        let coverage = idempotency_manifest_coverage(&dialect_manifest.entries);
        assert!(
            coverage.ported >= 25,
            "dialect '{}' must have at least 25 ported idempotency cases, found {}",
            dialect_name,
            coverage.ported
        );
        assert!(
            coverage.coverage_rate >= 0.70,
            "dialect '{}' idempotency coverage must be >= 70%, got {:.2}%",
            dialect_name,
            coverage.coverage_rate * 100.0
        );

        assert_manifest_tracks_all_ported_cases(dialect_name, &dialect_manifest.entries);
    }
}

fn assert_manifest_tracks_all_ported_cases(
    dialect_name: &str,
    entries: &[IdempotencyManifestEntry],
) {
    let manifest_ported_cases = manifest_ported_case_references(entries).unwrap_or_else(|error| {
        panic!(
            "failed to collect manifest case references for dialect '{}': {}",
            dialect_name, error
        )
    });

    let files = load_test_cases_from_dir(idempotency_root(dialect_name)).unwrap_or_else(|error| {
        panic!(
            "failed to load idempotency files for dialect '{}': {}",
            dialect_name, error
        )
    });
    let actual_cases = yaml_case_references(&files);

    assert_eq!(
        manifest_ported_cases, actual_cases,
        "manifest ported cases must exactly match YAML corpus for dialect '{}'",
        dialect_name
    );
}

fn idempotency_manifest_path() -> PathBuf {
    workspace_root().join("tests/migration/idempotency-manifest.yml")
}

fn idempotency_root(dialect_name: &str) -> PathBuf {
    workspace_root()
        .join("tests")
        .join(dialect_name)
        .join("idempotency")
}

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}
