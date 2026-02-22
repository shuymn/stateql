#[path = "support/ir_pattern_fixtures.rs"]
mod ir_pattern_fixtures;

use ir_pattern_fixtures::{qualified, sample_column};
use stateql_core::{
    ColumnChange, ColumnPosition, DataType, DiffOp, Expr, GeneratedColumn, Ident, Identity,
    IndexOwner, Literal, Value, extra_keys, is_mysql_change_column_full_redefinition,
};

#[test]
fn mysql_change_column_full_redefinition_is_representable() {
    let op = DiffOp::AlterColumn {
        table: qualified(Some("public"), "users"),
        column: Ident::unquoted("email"),
        changes: vec![
            ColumnChange::SetType(DataType::Varchar { length: Some(320) }),
            ColumnChange::SetNotNull(true),
            ColumnChange::SetDefault(Some(Expr::Literal(Literal::String(
                "unknown@example.com".to_string(),
            )))),
            ColumnChange::SetIdentity(Some(Identity {
                always: false,
                start: Some(1),
                increment: Some(1),
                min_value: None,
                max_value: None,
                cache: None,
                cycle: false,
            })),
            ColumnChange::SetGenerated(Some(GeneratedColumn {
                expr: Expr::Raw("lower(email)".to_string()),
                stored: true,
            })),
            ColumnChange::SetCollation(Some("und-x-icu".to_string())),
        ],
    };

    let changes = match op {
        DiffOp::AlterColumn { changes, .. } => changes,
        _ => panic!("expected DiffOp::AlterColumn"),
    };
    assert!(is_mysql_change_column_full_redefinition(&changes));
    assert!(
        changes
            .iter()
            .any(|change| matches!(change, ColumnChange::SetType(_)))
    );
    assert!(
        changes
            .iter()
            .any(|change| matches!(change, ColumnChange::SetNotNull(_)))
    );
    assert!(
        changes
            .iter()
            .any(|change| matches!(change, ColumnChange::SetDefault(_)))
    );
    assert!(
        changes
            .iter()
            .any(|change| matches!(change, ColumnChange::SetIdentity(_)))
    );
    assert!(
        changes
            .iter()
            .any(|change| matches!(change, ColumnChange::SetGenerated(_)))
    );
    assert!(
        changes
            .iter()
            .any(|change| matches!(change, ColumnChange::SetCollation(_)))
    );
}

#[test]
fn mysql_after_column_position_is_representable() {
    let op = DiffOp::AddColumn {
        table: qualified(Some("public"), "users"),
        column: Box::new(sample_column("display_name")),
        position: Some(ColumnPosition::After(Ident::unquoted("email"))),
    };

    assert!(matches!(
        op,
        DiffOp::AddColumn {
            position: Some(ColumnPosition::After(after)),
            ..
        } if after == Ident::unquoted("email")
    ));
}

#[test]
fn mssql_named_default_constraint_is_representable_via_column_extra() {
    let mut column = sample_column("created_at");
    column.extra.insert(
        extra_keys::mssql::DEFAULT_CONSTRAINT_NAME.to_string(),
        Value::String("DF_users_created_at".to_string()),
    );

    assert_eq!(
        column.extra.get(extra_keys::mssql::DEFAULT_CONSTRAINT_NAME),
        Some(&Value::String("DF_users_created_at".to_string())),
    );
}

#[test]
fn mssql_rename_operations_are_representable() {
    let from_table = qualified(Some("dbo"), "users_old");
    let to_table = qualified(Some("dbo"), "users");
    let target_table = qualified(Some("dbo"), "users");

    let rename_table = DiffOp::RenameTable {
        from: from_table.clone(),
        to: to_table.clone(),
    };
    let rename_column = DiffOp::RenameColumn {
        table: target_table.clone(),
        from: Ident::unquoted("old_name"),
        to: Ident::unquoted("new_name"),
    };
    let rename_index = DiffOp::RenameIndex {
        owner: IndexOwner::Table(target_table.clone()),
        from: Ident::unquoted("ix_old_name"),
        to: Ident::unquoted("ix_new_name"),
    };

    assert!(matches!(
        rename_table,
        DiffOp::RenameTable { from, to } if from == from_table && to == to_table
    ));
    assert!(matches!(
        rename_column,
        DiffOp::RenameColumn { table, from, to }
            if table == target_table
                && from == Ident::unquoted("old_name")
                && to == Ident::unquoted("new_name")
    ));
    assert!(matches!(
        rename_index,
        DiffOp::RenameIndex { owner: IndexOwner::Table(table), from, to }
            if table == target_table
                && from == Ident::unquoted("ix_old_name")
                && to == Ident::unquoted("ix_new_name")
    ));
}

#[test]
fn mysql_auto_increment_metadata_is_representable_via_column_extra() {
    let mut column = sample_column("id");
    column.extra.insert(
        extra_keys::mysql::AUTO_INCREMENT.to_string(),
        Value::Bool(true),
    );

    assert_eq!(
        column.extra.get(extra_keys::mysql::AUTO_INCREMENT),
        Some(&Value::Bool(true)),
    );
}
