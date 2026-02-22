use std::collections::{BTreeMap, BTreeSet};

#[path = "support/diffop_fixtures.rs"]
mod diffop_fixtures;

use diffop_fixtures::{all_diffop_variants, diffop_variant_tag};
use stateql_core::{
    CheckConstraint, Column, ColumnChange, ColumnPosition, DataType, DiffOp, Expr, ForeignKey,
    Ident, IndexColumn, IndexDef, IndexOwner, Partition, PartitionStrategy, PrimaryKey,
    QualifiedName, Table, TableOptions, View, sort_diff_ops,
};

fn ident(value: &str) -> Ident {
    Ident::unquoted(value)
}

fn qualified(schema: Option<&str>, name: &str) -> QualifiedName {
    QualifiedName {
        schema: schema.map(Ident::unquoted),
        name: Ident::unquoted(name),
    }
}

fn table(name: &str) -> Table {
    Table {
        name: qualified(Some("public"), name),
        columns: Vec::new(),
        primary_key: None,
        foreign_keys: Vec::new(),
        checks: Vec::new(),
        exclusions: Vec::new(),
        options: TableOptions::default(),
        partition: None,
        renamed_from: None,
    }
}

fn table_with_fk(name: &str, referenced: &str) -> Table {
    let mut table = table(name);
    table.foreign_keys.push(ForeignKey {
        name: Some(ident(&format!("{}_{}_fk", name, referenced))),
        columns: vec![ident("parent_id")],
        referenced_table: qualified(Some("public"), referenced),
        referenced_columns: vec![ident("id")],
        on_delete: None,
        on_update: None,
        deferrable: None,
        extra: BTreeMap::new(),
    });
    table
}

fn view(name: &str, query: &str) -> View {
    View {
        name: qualified(Some("public"), name),
        columns: Vec::new(),
        query: query.to_string(),
        check_option: None,
        security: None,
        renamed_from: None,
    }
}

fn index(name: &str, owner_table: &str) -> IndexDef {
    IndexDef {
        name: Some(ident(name)),
        owner: IndexOwner::Table(qualified(Some("public"), owner_table)),
        columns: vec![IndexColumn {
            expr: Expr::Ident(ident("id")),
        }],
        unique: false,
        method: Some("btree".to_string()),
        where_clause: None,
        concurrent: false,
        extra: BTreeMap::new(),
    }
}

fn by_tag() -> BTreeMap<&'static str, DiffOp> {
    all_diffop_variants()
        .into_iter()
        .map(|op| (diffop_variant_tag(&op), op))
        .collect()
}

fn design_priority(op: &DiffOp) -> u8 {
    match op {
        DiffOp::DropPolicy { .. } => 1,
        DiffOp::DropTrigger { .. } => 2,
        DiffOp::DropView(_) | DiffOp::DropMaterializedView(_) => 3,
        DiffOp::DropForeignKey { .. } => 4,
        DiffOp::DropIndex { .. } => 5,
        DiffOp::DropTable(_) => 6,
        DiffOp::DropSequence(_) => 7,
        DiffOp::DropDomain(_) => 8,
        DiffOp::DropType(_) => 9,
        DiffOp::DropFunction(_) => 10,
        DiffOp::DropSchema(_) => 11,
        DiffOp::DropExtension(_) => 12,
        DiffOp::CreateExtension(_) => 13,
        DiffOp::CreateSchema(_) => 14,
        DiffOp::CreateType(_) => 15,
        DiffOp::AlterType { .. } => 16,
        DiffOp::CreateDomain(_) => 17,
        DiffOp::AlterDomain { .. } => 18,
        DiffOp::CreateSequence(_) => 19,
        DiffOp::AlterSequence { .. } => 20,
        DiffOp::CreateTable(_) => 21,
        DiffOp::RenameTable { .. }
        | DiffOp::RenameColumn { .. }
        | DiffOp::AlterColumn { .. }
        | DiffOp::AddColumn { .. }
        | DiffOp::DropColumn { .. }
        | DiffOp::SetPrimaryKey { .. }
        | DiffOp::DropPrimaryKey { .. }
        | DiffOp::AddCheck { .. }
        | DiffOp::DropCheck { .. }
        | DiffOp::AddExclusion { .. }
        | DiffOp::DropExclusion { .. }
        | DiffOp::AddPartition { .. }
        | DiffOp::DropPartition { .. }
        | DiffOp::AlterTableOptions { .. } => 22,
        DiffOp::AddForeignKey { .. } => 23,
        DiffOp::CreateView(_) => 24,
        DiffOp::CreateMaterializedView(_) => 25,
        DiffOp::AddIndex(_) | DiffOp::RenameIndex { .. } => 26,
        DiffOp::CreateTrigger(_) | DiffOp::CreateFunction(_) => 27,
        DiffOp::CreatePolicy(_) => 28,
        DiffOp::SetComment(_) | DiffOp::DropComment { .. } => 29,
        DiffOp::Grant(_) | DiffOp::Revoke(_) => 30,
    }
}

#[test]
fn sorts_using_design_priority_groups_1_through_30() {
    let ops_by_tag = by_tag();

    let design_fixture: Vec<(u8, &'static [&'static str])> = vec![
        (1, &["DropPolicy"]),
        (2, &["DropTrigger"]),
        (3, &["DropView", "DropMaterializedView"]),
        (4, &["DropForeignKey"]),
        (5, &["DropIndex"]),
        (6, &["DropTable"]),
        (7, &["DropSequence"]),
        (8, &["DropDomain"]),
        (9, &["DropType"]),
        (10, &["DropFunction"]),
        (11, &["DropSchema"]),
        (12, &["DropExtension"]),
        (13, &["CreateExtension"]),
        (14, &["CreateSchema"]),
        (15, &["CreateType"]),
        (16, &["AlterType"]),
        (17, &["CreateDomain"]),
        (18, &["AlterDomain"]),
        (19, &["CreateSequence"]),
        (20, &["AlterSequence"]),
        (21, &["CreateTable"]),
        (22, &["RenameTable"]),
        (23, &["AddForeignKey"]),
        (24, &["CreateView"]),
        (25, &["CreateMaterializedView"]),
        (26, &["AddIndex", "RenameIndex"]),
        (27, &["CreateTrigger", "CreateFunction"]),
        (28, &["CreatePolicy"]),
        (29, &["SetComment", "DropComment"]),
        (30, &["Grant", "Revoke"]),
    ];

    let mut unsorted = Vec::new();
    for (_, tags) in design_fixture.iter().rev() {
        for tag in tags.iter().rev() {
            unsorted.push(
                ops_by_tag
                    .get(tag)
                    .unwrap_or_else(|| panic!("missing fixture op for {tag}"))
                    .clone(),
            );
        }
    }

    let sorted = sort_diff_ops(unsorted);
    let priorities = sorted.iter().map(design_priority).collect::<Vec<_>>();

    for pair in priorities.windows(2) {
        assert!(
            pair[0] <= pair[1],
            "priorities must be non-decreasing, got {:?}",
            priorities
        );
    }

    let present = priorities.iter().copied().collect::<BTreeSet<_>>();
    for expected_priority in 1..=30 {
        assert!(
            present.contains(&expected_priority),
            "missing priority {expected_priority} in sorted output"
        );
    }

    for (priority, tags) in design_fixture {
        let group_tags = sorted
            .iter()
            .filter(|op| design_priority(op) == priority)
            .map(diffop_variant_tag)
            .collect::<BTreeSet<_>>();

        for tag in tags {
            assert!(
                group_tags.contains(tag),
                "priority {priority} fixture must include {tag}"
            );
        }
    }
}

#[test]
fn sorts_intra_table_priority_22_subpriorities() {
    let table_name = qualified(Some("public"), "users");

    let ops = vec![
        DiffOp::AlterTableOptions {
            table: table_name.clone(),
            options: TableOptions::default(),
        },
        DiffOp::AddPartition {
            table: table_name.clone(),
            partition: Partition {
                strategy: PartitionStrategy::Range,
                columns: vec![ident("created_at")],
                partitions: Vec::new(),
            },
        },
        DiffOp::AddCheck {
            table: table_name.clone(),
            check: CheckConstraint {
                name: Some(ident("users_age_check")),
                expr: Expr::Raw("age > 0".to_string()),
                no_inherit: false,
            },
        },
        DiffOp::SetPrimaryKey {
            table: table_name.clone(),
            pk: PrimaryKey {
                name: Some(ident("users_pkey")),
                columns: vec![ident("id")],
            },
        },
        DiffOp::DropColumn {
            table: table_name.clone(),
            column: ident("legacy_col"),
        },
        DiffOp::AddColumn {
            table: table_name.clone(),
            column: Box::new(Column {
                name: ident("nickname"),
                data_type: DataType::Text,
                not_null: false,
                default: None,
                identity: None,
                generated: None,
                comment: None,
                collation: None,
                renamed_from: None,
                extra: BTreeMap::new(),
            }),
            position: Some(ColumnPosition::First),
        },
        DiffOp::AlterColumn {
            table: table_name.clone(),
            column: ident("email"),
            changes: vec![ColumnChange::SetType(DataType::Varchar {
                length: Some(255),
            })],
        },
        DiffOp::RenameColumn {
            table: table_name.clone(),
            from: ident("full_name"),
            to: ident("display_name"),
        },
        DiffOp::RenameTable {
            from: qualified(Some("public"), "legacy_users"),
            to: table_name,
        },
    ];

    let sorted_tags = sort_diff_ops(ops)
        .iter()
        .map(diffop_variant_tag)
        .collect::<Vec<_>>();

    assert_eq!(
        sorted_tags,
        vec![
            "RenameTable",
            "RenameColumn",
            "AlterColumn",
            "AddColumn",
            "DropColumn",
            "SetPrimaryKey",
            "AddCheck",
            "AddPartition",
            "AlterTableOptions",
        ],
    );
}

#[test]
fn sorts_create_table_by_fk_dependency_within_same_priority() {
    let child = table_with_fk("child", "parent");
    let parent = table("parent");

    let sorted = sort_diff_ops(vec![
        DiffOp::CreateTable(child),
        DiffOp::CreateTable(parent.clone()),
    ]);

    let create_table_names = sorted
        .iter()
        .filter_map(|op| match op {
            DiffOp::CreateTable(table) => Some(table.name.name.value.as_str()),
            _ => None,
        })
        .collect::<Vec<_>>();

    assert_eq!(create_table_names, vec!["parent", "child"]);
}

#[test]
fn sorts_create_view_by_dependency_within_same_priority() {
    let dependent = view("v_dependent", "SELECT id FROM public.v_base");
    let base = view("v_base", "SELECT id FROM public.users");

    let sorted = sort_diff_ops(vec![
        DiffOp::CreateView(dependent),
        DiffOp::CreateView(base.clone()),
    ]);

    let create_view_names = sorted
        .iter()
        .filter_map(|op| match op {
            DiffOp::CreateView(view) => Some(view.name.name.value.as_str()),
            _ => None,
        })
        .collect::<Vec<_>>();

    assert_eq!(create_view_names, vec!["v_base", "v_dependent"]);
}

#[test]
fn preserves_declaration_order_for_independent_same_priority_ops() {
    let second = DiffOp::AddIndex(index("idx_orders_created_at", "orders"));
    let first = DiffOp::AddIndex(index("idx_users_created_at", "users"));

    let sorted = sort_diff_ops(vec![second.clone(), first.clone()]);

    assert_eq!(sorted, vec![second, first]);
}

#[test]
fn diffop_fixture_exports_are_consumed_in_ordering_tests() {
    let _ = diffop_fixtures::EXPECTED_DIFFOP_VARIANT_COUNT;
    let _ = diffop_fixtures::EXPECTED_COLUMN_CHANGE_VARIANT_COUNT;
    let _ = diffop_fixtures::EXPECTED_SEQUENCE_CHANGE_VARIANT_COUNT;
    let _ = diffop_fixtures::EXPECTED_TYPE_CHANGE_VARIANT_COUNT;
    let _ = diffop_fixtures::EXPECTED_DOMAIN_CHANGE_VARIANT_COUNT;

    let column_change = diffop_fixtures::all_column_change_variants()
        .into_iter()
        .next()
        .expect("column change fixture should have at least one variant");
    let sequence_change = diffop_fixtures::all_sequence_change_variants()
        .into_iter()
        .next()
        .expect("sequence change fixture should have at least one variant");
    let type_change = diffop_fixtures::all_type_change_variants()
        .into_iter()
        .next()
        .expect("type change fixture should have at least one variant");
    let domain_change = diffop_fixtures::all_domain_change_variants()
        .into_iter()
        .next()
        .expect("domain change fixture should have at least one variant");

    let _ = diffop_fixtures::column_change_variant_tag(&column_change);
    let _ = diffop_fixtures::sequence_change_variant_tag(&sequence_change);
    let _ = diffop_fixtures::type_change_variant_tag(&type_change);
    let _ = diffop_fixtures::domain_change_variant_tag(&domain_change);
}
