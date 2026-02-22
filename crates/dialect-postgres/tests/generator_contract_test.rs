use std::collections::BTreeSet;

#[allow(dead_code)]
#[path = "../../core/tests/support/diffop_fixtures.rs"]
mod diffop_fixtures;

use diffop_fixtures::{EXPECTED_DIFFOP_VARIANT_COUNT, all_diffop_variants, diffop_variant_tag};
use stateql_core::{
    Dialect, DiffOp, Error, GenerateError, Ident, IndexDef, IndexOwner, QualifiedName,
};
use stateql_dialect_postgres::PostgresDialect;

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
        "all_diffop_variants() should not include duplicate variants",
    );

    for op in &variants {
        let _ = is_supported_diffop(op);
    }
}

#[test]
fn every_diffop_variant_matches_support_contract() {
    let dialect = PostgresDialect;

    for op in &all_diffop_variants() {
        if is_supported_diffop(op) {
            let statements = dialect
                .generate_ddl(std::slice::from_ref(op))
                .unwrap_or_else(|error| panic!("expected supported op to generate SQL: {error:?}"));
            assert!(
                !statements.is_empty(),
                "supported op should emit at least one SQL statement"
            );
        } else {
            let error = dialect
                .generate_ddl(std::slice::from_ref(op))
                .expect_err("unsupported op should return GenerateError");
            assert_unsupported_diffop_error(error, diffop_variant_tag(op));
        }
    }
}

#[test]
fn malformed_payload_is_rejected_with_generate_error() {
    let dialect = PostgresDialect;
    let op = DiffOp::AddIndex(IndexDef {
        name: None,
        owner: IndexOwner::Table(qualified(Some("public"), "users")),
        columns: Vec::new(),
        unique: false,
        method: None,
        where_clause: None,
        concurrent: false,
        extra: Default::default(),
    });

    let error = dialect
        .generate_ddl(&[op])
        .expect_err("invalid index payload should fail");

    assert_unsupported_diffop_error(error, "AddIndex");
}

fn is_supported_diffop(op: &DiffOp) -> bool {
    match op {
        DiffOp::CreateTable(_)
        | DiffOp::DropTable(_)
        | DiffOp::RenameTable { .. }
        | DiffOp::AddColumn { .. }
        | DiffOp::DropColumn { .. }
        | DiffOp::AlterColumn { .. }
        | DiffOp::RenameColumn { .. }
        | DiffOp::AddIndex(_)
        | DiffOp::DropIndex { .. }
        | DiffOp::RenameIndex { .. }
        | DiffOp::AddForeignKey { .. }
        | DiffOp::DropForeignKey { .. }
        | DiffOp::AddCheck { .. }
        | DiffOp::DropCheck { .. }
        | DiffOp::AddExclusion { .. }
        | DiffOp::DropExclusion { .. }
        | DiffOp::SetPrimaryKey { .. }
        | DiffOp::DropPrimaryKey { .. }
        | DiffOp::AddPartition { .. }
        | DiffOp::DropPartition { .. }
        | DiffOp::CreateView(_)
        | DiffOp::DropView(_)
        | DiffOp::CreateMaterializedView(_)
        | DiffOp::DropMaterializedView(_)
        | DiffOp::CreateSequence(_)
        | DiffOp::DropSequence(_)
        | DiffOp::AlterSequence { .. }
        | DiffOp::CreateTrigger(_)
        | DiffOp::DropTrigger { .. }
        | DiffOp::CreateFunction(_)
        | DiffOp::DropFunction(_)
        | DiffOp::CreateType(_)
        | DiffOp::DropType(_)
        | DiffOp::AlterType { .. }
        | DiffOp::CreateDomain(_)
        | DiffOp::DropDomain(_)
        | DiffOp::AlterDomain { .. }
        | DiffOp::CreateExtension(_)
        | DiffOp::DropExtension(_)
        | DiffOp::CreateSchema(_)
        | DiffOp::DropSchema(_)
        | DiffOp::SetComment(_)
        | DiffOp::DropComment { .. }
        | DiffOp::Grant(_)
        | DiffOp::Revoke(_)
        | DiffOp::CreatePolicy(_)
        | DiffOp::DropPolicy { .. }
        | DiffOp::AlterTableOptions { .. } => true,
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

fn qualified(schema: Option<&str>, name: &str) -> QualifiedName {
    QualifiedName {
        schema: schema.map(Ident::unquoted),
        name: Ident::unquoted(name),
    }
}
