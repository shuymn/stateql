use std::collections::BTreeSet;

#[allow(dead_code)]
#[path = "../../core/tests/support/diffop_fixtures.rs"]
mod diffop_fixtures;

use diffop_fixtures::{EXPECTED_DIFFOP_VARIANT_COUNT, all_diffop_variants, diffop_variant_tag};
use stateql_core::{Dialect, DiffOp, Error, GenerateError, IndexOwner};
use stateql_dialect_sqlite::SqliteDialect;

#[test]
fn all_diffop_variants_are_exhaustively_classified() {
    let variants = all_diffop_variants();
    let tags = variants
        .iter()
        .map(diffop_variant_tag)
        .collect::<BTreeSet<_>>();

    assert_eq!(
        variants.len(),
        EXPECTED_DIFFOP_VARIANT_COUNT,
        "all_diffop_variants() should include every DiffOp variant",
    );
    assert_eq!(
        tags.len(),
        variants.len(),
        "all_diffop_variants() should not contain duplicate variants",
    );

    for op in &variants {
        let _ = is_supported_diffop(op);
    }
}

#[test]
fn every_diffop_variant_matches_sqlite_support_contract() {
    let dialect = SqliteDialect;

    for op in &all_diffop_variants() {
        if is_supported_diffop(op) {
            let statements = dialect
                .generate_ddl(std::slice::from_ref(op))
                .unwrap_or_else(|error| panic!("expected supported op to generate SQL: {error:?}"));
            assert!(
                !statements.is_empty(),
                "supported op should emit at least one statement"
            );
        } else {
            let error = dialect
                .generate_ddl(std::slice::from_ref(op))
                .expect_err("unsupported op should return GenerateError");
            assert_unsupported_diffop_error(error, diffop_variant_tag(op));
        }
    }
}

fn is_supported_diffop(op: &DiffOp) -> bool {
    match op {
        DiffOp::CreateTable(table) => table.exclusions.is_empty() && table.partition.is_none(),
        DiffOp::DropTable(_) => true,
        DiffOp::RenameTable { from, to } => from.schema == to.schema,
        DiffOp::AddColumn { position, .. } => position.is_none(),
        DiffOp::DropColumn { .. } => true,
        DiffOp::AlterColumn { changes, .. } => !changes.is_empty(),
        DiffOp::RenameColumn { .. } => true,
        DiffOp::AddIndex(index) => {
            index.name.is_some() && matches!(index.owner, IndexOwner::Table(_))
        }
        DiffOp::DropIndex { owner, .. } => matches!(owner, IndexOwner::Table(_)),
        DiffOp::RenameIndex { .. } => false,
        DiffOp::AddForeignKey { .. } => true,
        DiffOp::DropForeignKey { .. } => true,
        DiffOp::AddCheck { .. } => true,
        DiffOp::DropCheck { .. } => true,
        DiffOp::AddExclusion { .. } => true,
        DiffOp::DropExclusion { .. } => true,
        DiffOp::SetPrimaryKey { .. } => true,
        DiffOp::DropPrimaryKey { .. } => true,
        DiffOp::AddPartition { .. } => false,
        DiffOp::DropPartition { .. } => false,
        DiffOp::CreateView(_) => true,
        DiffOp::DropView(_) => true,
        DiffOp::CreateMaterializedView(_) => false,
        DiffOp::DropMaterializedView(_) => false,
        DiffOp::CreateSequence(_) => false,
        DiffOp::DropSequence(_) => false,
        DiffOp::AlterSequence { .. } => false,
        DiffOp::CreateTrigger(trigger) => !trigger.events.is_empty(),
        DiffOp::DropTrigger { .. } => true,
        DiffOp::CreateFunction(_) => false,
        DiffOp::DropFunction(_) => false,
        DiffOp::CreateType(_) => false,
        DiffOp::DropType(_) => false,
        DiffOp::AlterType { .. } => false,
        DiffOp::CreateDomain(_) => false,
        DiffOp::DropDomain(_) => false,
        DiffOp::AlterDomain { .. } => false,
        DiffOp::CreateExtension(_) => false,
        DiffOp::DropExtension(_) => false,
        DiffOp::CreateSchema(_) => false,
        DiffOp::DropSchema(_) => false,
        DiffOp::SetComment(_) => false,
        DiffOp::DropComment { .. } => false,
        DiffOp::Grant(_) => false,
        DiffOp::Revoke(_) => false,
        DiffOp::CreatePolicy(_) => false,
        DiffOp::DropPolicy { .. } => false,
        DiffOp::AlterTableOptions { .. } => false,
    }
}

fn assert_unsupported_diffop_error(error: Error, expected_diff_op: &str) {
    match error {
        Error::Generate(GenerateError::UnsupportedDiffOp { diff_op, .. }) => {
            assert_eq!(diff_op, expected_diff_op);
        }
        other => panic!("expected unsupported diff op error, got {other:?}"),
    }
}
