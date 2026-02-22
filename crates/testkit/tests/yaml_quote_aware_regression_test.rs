use std::{fs, path::PathBuf};

use stateql_testkit::{
    ManifestStatus, load_assertion_manifest_from_path, load_idempotency_manifest_from_path,
    sql_matches_quote_aware,
};

#[test]
fn legacy_ignore_quotes_manifest_entries_are_rewritten() {
    let mut unresolved_entries = Vec::new();

    let idempotency_manifest = load_idempotency_manifest_from_path(
        workspace_root().join("tests/migration/idempotency-manifest.yml"),
    )
    .unwrap_or_else(|error| panic!("failed to load idempotency manifest: {error}"));

    for (dialect_name, dialect_manifest) in idempotency_manifest.dialects {
        for entry in dialect_manifest.entries {
            if entry.id.contains("LegacyIgnoreQuotes") && entry.status == ManifestStatus::Skipped {
                unresolved_entries.push(format!("idempotency::{dialect_name}::{}", entry.id));
            }
        }
    }

    let assertion_manifest = load_assertion_manifest_from_path(
        workspace_root().join("tests/migration/assertion-manifest.yml"),
    )
    .unwrap_or_else(|error| panic!("failed to load assertion manifest: {error}"));

    for (dialect_name, dialect_manifest) in assertion_manifest.dialects {
        for (group_name, group_manifest) in dialect_manifest.groups {
            for entry in group_manifest.entries {
                if entry.id.contains("LegacyIgnoreQuotes")
                    && entry.status == ManifestStatus::Skipped
                {
                    unresolved_entries.push(format!(
                        "assertion::{dialect_name}::{group_name}::{}",
                        entry.id
                    ));
                }
            }
        }
    }

    assert!(
        unresolved_entries.is_empty(),
        "legacy_ignore_quotes-dependent entries must be rewritten to quote-aware cases, unresolved entries: {unresolved_entries:?}"
    );
}

#[test]
fn yaml_corpus_does_not_use_legacy_ignore_quotes_field() {
    let mut offenders = Vec::new();

    for path in collect_yaml_paths(workspace_root().join("tests")) {
        let content = fs::read_to_string(&path)
            .unwrap_or_else(|error| panic!("failed to read '{}': {error}", path.display()));
        if content.contains("legacy_ignore_quotes:") {
            offenders.push(path.display().to_string());
        }
    }

    assert!(
        offenders.is_empty(),
        "YAML corpus must not contain legacy_ignore_quotes field, offenders: {offenders:?}"
    );
}

#[test]
fn quote_aware_sql_comparison_does_not_ignore_identifier_quotes() {
    let expected = r#"CREATE TABLE "QuotedName" ("QuotedColumn" BIGINT);"#;
    let same_with_spacing = r#"CREATE     TABLE "QuotedName"   ("QuotedColumn"    BIGINT);"#;
    let without_quotes = "CREATE TABLE QuotedName (QuotedColumn BIGINT);";

    assert!(
        sql_matches_quote_aware(expected, same_with_spacing),
        "quote-aware comparison should ignore whitespace-only differences"
    );
    assert!(
        !sql_matches_quote_aware(expected, without_quotes),
        "quote-aware comparison must treat quoted and unquoted identifiers as different"
    );
}

fn collect_yaml_paths(root: PathBuf) -> Vec<PathBuf> {
    let mut paths = Vec::new();
    collect_yaml_paths_recursive(&root, &mut paths);
    paths.sort();
    paths
}

fn collect_yaml_paths_recursive(root: &PathBuf, out: &mut Vec<PathBuf>) {
    let entries = fs::read_dir(root)
        .unwrap_or_else(|error| panic!("failed to read '{}': {error}", root.display()));

    for entry in entries {
        let entry = entry.unwrap_or_else(|error| {
            panic!(
                "failed to read directory entry under '{}': {error}",
                root.display()
            )
        });
        let path = entry.path();
        if path.is_dir() {
            collect_yaml_paths_recursive(&path, out);
            continue;
        }

        if matches!(
            path.extension().and_then(|ext| ext.to_str()),
            Some("yml" | "yaml")
        ) {
            out.push(path);
        }
    }
}

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}
