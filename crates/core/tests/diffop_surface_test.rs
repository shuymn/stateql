use std::collections::BTreeSet;

#[path = "support/diffop_fixtures.rs"]
mod diffop_fixtures;

use diffop_fixtures::{
    EXPECTED_COLUMN_CHANGE_VARIANT_COUNT, EXPECTED_DIFFOP_VARIANT_COUNT,
    EXPECTED_DOMAIN_CHANGE_VARIANT_COUNT, EXPECTED_SEQUENCE_CHANGE_VARIANT_COUNT,
    EXPECTED_TYPE_CHANGE_VARIANT_COUNT, all_column_change_variants, all_diffop_variants,
    all_domain_change_variants, all_sequence_change_variants, all_type_change_variants,
    column_change_variant_tag, diffop_variant_tag, domain_change_variant_tag,
    sequence_change_variant_tag, type_change_variant_tag,
};
use stateql_core::DiffOp;

#[test]
fn diffop_surface_includes_major_variants() {
    let variants = all_diffop_variants();

    assert!(
        variants
            .iter()
            .any(|op| matches!(op, DiffOp::DropForeignKey { .. }))
    );
    assert!(variants.iter().any(|op| matches!(op, DiffOp::Grant(_))));
    assert!(
        variants
            .iter()
            .any(|op| matches!(op, DiffOp::AlterDomain { .. }))
    );
}

#[test]
fn all_diffop_variants_are_listed_once() {
    let variants = all_diffop_variants();
    let tags = variants
        .iter()
        .map(diffop_variant_tag)
        .collect::<BTreeSet<_>>();

    assert_eq!(
        variants.len(),
        EXPECTED_DIFFOP_VARIANT_COUNT,
        "all_diffop_variants() must list every DiffOp variant",
    );
    assert_eq!(
        tags.len(),
        variants.len(),
        "all_diffop_variants() must not include duplicates",
    );
}

#[test]
fn change_enum_surfaces_are_listed_once() {
    let column_changes = all_column_change_variants();
    let sequence_changes = all_sequence_change_variants();
    let type_changes = all_type_change_variants();
    let domain_changes = all_domain_change_variants();

    assert_eq!(
        column_changes
            .iter()
            .map(column_change_variant_tag)
            .collect::<BTreeSet<_>>()
            .len(),
        EXPECTED_COLUMN_CHANGE_VARIANT_COUNT,
    );
    assert_eq!(
        sequence_changes
            .iter()
            .map(sequence_change_variant_tag)
            .collect::<BTreeSet<_>>()
            .len(),
        EXPECTED_SEQUENCE_CHANGE_VARIANT_COUNT,
    );
    assert_eq!(
        type_changes
            .iter()
            .map(type_change_variant_tag)
            .collect::<BTreeSet<_>>()
            .len(),
        EXPECTED_TYPE_CHANGE_VARIANT_COUNT,
    );
    assert_eq!(
        domain_changes
            .iter()
            .map(domain_change_variant_tag)
            .collect::<BTreeSet<_>>()
            .len(),
        EXPECTED_DOMAIN_CHANGE_VARIANT_COUNT,
    );
}
