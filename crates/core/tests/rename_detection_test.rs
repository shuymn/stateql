use std::collections::BTreeMap;

use stateql_core::{
    Column, DataType, DiffConfig, DiffEngine, DiffOp, Expr, Ident, IndexColumn, IndexDef,
    IndexOwner, QualifiedName, SchemaObject, Table, Value,
};

const INDEX_RENAMED_FROM_EXTRA_KEY: &str = "stateql.renamed_from";

fn ident(value: &str) -> Ident {
    Ident::unquoted(value)
}

fn qualified(name: &str) -> QualifiedName {
    QualifiedName {
        schema: Some(ident("public")),
        name: ident(name),
    }
}

fn with_enable_drop(enable_drop: bool) -> DiffConfig {
    DiffConfig {
        enable_drop,
        ..DiffConfig::default()
    }
}

fn column(name: &str, renamed_from: Option<&str>) -> Column {
    Column {
        name: ident(name),
        data_type: DataType::BigInt,
        not_null: true,
        default: None,
        identity: None,
        generated: None,
        comment: None,
        collation: None,
        renamed_from: renamed_from.map(ident),
        extra: BTreeMap::new(),
    }
}

fn table(name: &str, renamed_from: Option<&str>, columns: Vec<Column>) -> Table {
    Table {
        name: qualified(name),
        columns,
        primary_key: None,
        foreign_keys: Vec::new(),
        checks: Vec::new(),
        exclusions: Vec::new(),
        options: Default::default(),
        partition: None,
        renamed_from: renamed_from.map(ident),
    }
}

fn index(owner_table: &str, name: &str, renamed_from: Option<&str>) -> IndexDef {
    let mut extra = BTreeMap::new();
    if let Some(from) = renamed_from {
        extra.insert(
            INDEX_RENAMED_FROM_EXTRA_KEY.to_string(),
            Value::String(from.to_string()),
        );
    }

    IndexDef {
        name: Some(ident(name)),
        owner: IndexOwner::Table(qualified(owner_table)),
        columns: vec![IndexColumn {
            expr: Expr::Ident(ident("id")),
        }],
        unique: false,
        method: Some("btree".to_string()),
        where_clause: None,
        concurrent: false,
        extra,
    }
}

#[test]
fn emits_rename_table_when_desired_table_has_renamed_from() {
    let engine = DiffEngine::new();

    let desired = vec![SchemaObject::Table(table(
        "users",
        Some("users_old"),
        vec![column("id", None)],
    ))];
    let current = vec![SchemaObject::Table(table(
        "users_old",
        None,
        vec![column("id", None)],
    ))];

    let ops = engine
        .diff(&desired, &current, &with_enable_drop(true))
        .expect("diff should succeed");

    assert_eq!(
        ops,
        vec![DiffOp::RenameTable {
            from: qualified("users_old"),
            to: qualified("users"),
        }]
    );
}

#[test]
fn emits_rename_column_when_desired_column_has_renamed_from() {
    let engine = DiffEngine::new();

    let desired = vec![SchemaObject::Table(table(
        "users",
        None,
        vec![column("user_id", Some("id"))],
    ))];
    let current = vec![SchemaObject::Table(table(
        "users",
        None,
        vec![column("id", None)],
    ))];

    let ops = engine
        .diff(&desired, &current, &with_enable_drop(true))
        .expect("diff should succeed");

    assert_eq!(
        ops,
        vec![DiffOp::RenameColumn {
            table: qualified("users"),
            from: ident("id"),
            to: ident("user_id"),
        }]
    );
}

#[test]
fn emits_rename_index_when_desired_index_has_renamed_from_metadata() {
    let engine = DiffEngine::new();

    let desired = vec![
        SchemaObject::Table(table("users", None, vec![column("id", None)])),
        SchemaObject::Index(index("users", "users_id_idx", Some("users_old_id_idx"))),
    ];
    let current = vec![
        SchemaObject::Table(table("users", None, vec![column("id", None)])),
        SchemaObject::Index(index("users", "users_old_id_idx", None)),
    ];

    let ops = engine
        .diff(&desired, &current, &with_enable_drop(true))
        .expect("diff should succeed");

    assert_eq!(
        ops,
        vec![DiffOp::RenameIndex {
            owner: IndexOwner::Table(qualified("users")),
            from: ident("users_old_id_idx"),
            to: ident("users_id_idx"),
        }]
    );
}

#[test]
fn does_not_emit_rename_ops_without_annotation_metadata() {
    let engine = DiffEngine::new();

    let table_ops = engine
        .diff(
            &[SchemaObject::Table(table(
                "users",
                None,
                vec![column("id", None)],
            ))],
            &[SchemaObject::Table(table(
                "users_old",
                None,
                vec![column("id", None)],
            ))],
            &with_enable_drop(true),
        )
        .expect("table diff should succeed");
    assert!(table_ops.contains(&DiffOp::CreateTable(table(
        "users",
        None,
        vec![column("id", None)],
    ))));
    assert!(table_ops.contains(&DiffOp::DropTable(qualified("users_old"))));
    assert!(
        !table_ops
            .iter()
            .any(|op| matches!(op, DiffOp::RenameTable { .. }))
    );

    let column_ops = engine
        .diff(
            &[SchemaObject::Table(table(
                "users",
                None,
                vec![column("user_id", None)],
            ))],
            &[SchemaObject::Table(table(
                "users",
                None,
                vec![column("id", None)],
            ))],
            &with_enable_drop(true),
        )
        .expect("column diff should succeed");
    assert!(column_ops.contains(&DiffOp::AddColumn {
        table: qualified("users"),
        column: Box::new(column("user_id", None)),
        position: None,
    }));
    assert!(column_ops.contains(&DiffOp::DropColumn {
        table: qualified("users"),
        column: ident("id"),
    }));
    assert!(
        !column_ops
            .iter()
            .any(|op| matches!(op, DiffOp::RenameColumn { .. }))
    );

    let index_ops = engine
        .diff(
            &[
                SchemaObject::Table(table("users", None, vec![column("id", None)])),
                SchemaObject::Index(index("users", "users_id_idx", None)),
            ],
            &[
                SchemaObject::Table(table("users", None, vec![column("id", None)])),
                SchemaObject::Index(index("users", "users_old_id_idx", None)),
            ],
            &with_enable_drop(true),
        )
        .expect("index diff should succeed");
    assert!(index_ops.contains(&DiffOp::AddIndex(index("users", "users_id_idx", None))));
    assert!(index_ops.contains(&DiffOp::DropIndex {
        owner: IndexOwner::Table(qualified("users")),
        name: ident("users_old_id_idx"),
    }));
    assert!(
        !index_ops
            .iter()
            .any(|op| matches!(op, DiffOp::RenameIndex { .. }))
    );
}
