use std::path::PathBuf;

use stateql_testkit::{
    assertion_manifest_coverage, load_assertion_manifest_from_path, load_test_cases_from_dir,
    manifest_ported_case_references, validate_assertion_manifest_entries, yaml_case_references,
};

const DIALECTS: [&str; 4] = ["postgres", "sqlite", "mysql", "mssql"];
const GROUPS: [&str; 4] = ["tables", "indexes", "constraints", "views"];

#[test]
fn assertion_manifest_meets_count_and_coverage_gates() {
    let manifest = load_assertion_manifest_from_path(assertion_manifest_path())
        .unwrap_or_else(|error| panic!("failed to load assertion manifest: {error}"));

    for dialect_name in DIALECTS {
        let dialect_manifest = manifest
            .dialects
            .get(dialect_name)
            .unwrap_or_else(|| panic!("manifest is missing dialect '{dialect_name}'"));

        for group_name in GROUPS {
            let group_manifest = dialect_manifest.groups.get(group_name).unwrap_or_else(|| {
                panic!("manifest is missing group '{group_name}' for dialect '{dialect_name}'")
            });

            validate_assertion_manifest_entries(&group_manifest.entries).unwrap_or_else(|error| {
                panic!(
                    "manifest entry validation failed for dialect '{}' group '{}': {}",
                    dialect_name, group_name, error
                )
            });

            let coverage = assertion_manifest_coverage(&group_manifest.entries);
            assert!(
                coverage.ported >= 5,
                "dialect '{}' group '{}' must have at least 5 ported assertion cases, found {}",
                dialect_name,
                group_name,
                coverage.ported
            );
            assert!(
                coverage.coverage_rate >= 0.70,
                "dialect '{}' group '{}' assertion coverage must be >= 70%, got {:.2}%",
                dialect_name,
                group_name,
                coverage.coverage_rate * 100.0
            );

            assert_manifest_tracks_all_ported_cases(
                dialect_name,
                group_name,
                &group_manifest.entries,
            );
        }
    }
}

fn assert_manifest_tracks_all_ported_cases(
    dialect_name: &str,
    group_name: &str,
    entries: &[stateql_testkit::IdempotencyManifestEntry],
) {
    let manifest_ported_cases = manifest_ported_case_references(entries).unwrap_or_else(|error| {
        panic!(
            "failed to collect manifest case references for dialect '{}' group '{}': {}",
            dialect_name, group_name, error
        )
    });

    let files = load_test_cases_from_dir(assertion_root(dialect_name, group_name)).unwrap_or_else(
        |error| {
            panic!(
                "failed to load assertion files for dialect '{}' group '{}': {}",
                dialect_name, group_name, error
            )
        },
    );

    let actual_cases = yaml_case_references(&files);

    assert_eq!(
        manifest_ported_cases, actual_cases,
        "manifest ported cases must exactly match YAML corpus for dialect '{}' group '{}'",
        dialect_name, group_name
    );
}

fn assertion_manifest_path() -> PathBuf {
    workspace_root().join("tests/migration/assertion-manifest.yml")
}

fn assertion_root(dialect_name: &str, group_name: &str) -> PathBuf {
    workspace_root()
        .join("tests")
        .join(dialect_name)
        .join("assertions")
        .join(group_name)
}

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}
