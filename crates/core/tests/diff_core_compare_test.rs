use std::{collections::BTreeMap, sync::Arc};

use stateql_core::{
    CheckConstraint, Column, ColumnChange, DataType, DiffConfig, DiffEngine, DiffError, DiffOp,
    EquivalencePolicy, Error, Expr, Ident, IndexColumn, IndexDef, IndexOwner, Literal,
    QualifiedName, SchemaObject, Table,
};

fn ident(value: &str) -> Ident {
    Ident::unquoted(value)
}

fn qualified(name: &str) -> QualifiedName {
    QualifiedName {
        schema: Some(ident("public")),
        name: ident(name),
    }
}

fn base_table(name: &str) -> Table {
    Table {
        name: qualified(name),
        columns: Vec::new(),
        primary_key: None,
        foreign_keys: Vec::new(),
        checks: Vec::new(),
        exclusions: Vec::new(),
        options: Default::default(),
        partition: None,
        renamed_from: None,
    }
}

fn column(name: &str, data_type: DataType, not_null: bool, default: Option<Expr>) -> Column {
    Column {
        name: ident(name),
        data_type,
        not_null,
        default,
        identity: None,
        generated: None,
        comment: None,
        collation: None,
        renamed_from: None,
        extra: BTreeMap::new(),
    }
}

fn table_index(table_name: &str, index_name: &str) -> IndexDef {
    IndexDef {
        name: Some(ident(index_name)),
        owner: IndexOwner::Table(qualified(table_name)),
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

fn with_enable_drop(enable_drop: bool) -> DiffConfig {
    DiffConfig {
        enable_drop,
        ..DiffConfig::default()
    }
}

#[derive(Debug)]
struct CastLiteralExprPolicy;

impl EquivalencePolicy for CastLiteralExprPolicy {
    fn is_equivalent_expr(&self, left: &Expr, right: &Expr) -> bool {
        matches!(
            (left, right),
            (Expr::Raw(raw), Expr::Literal(Literal::Integer(0))) if raw == "'0'::integer"
        ) || matches!(
            (left, right),
            (Expr::Literal(Literal::Integer(0)), Expr::Raw(raw)) if raw == "'0'::integer"
        ) || matches!(
            (left, right),
            (Expr::Raw(left_raw), Expr::Raw(right_raw))
                if left_raw == "quantity > '0'::integer" && right_raw == "quantity > 0"
        ) || matches!(
            (left, right),
            (Expr::Raw(left_raw), Expr::Raw(right_raw))
                if left_raw == "quantity > 0" && right_raw == "quantity > '0'::integer"
        )
    }
}

#[test]
fn creates_table_when_missing_in_current() {
    let engine = DiffEngine::new();
    let desired_table = base_table("users");
    let desired = vec![SchemaObject::Table(desired_table.clone())];
    let current = vec![];

    let ops = engine
        .diff(&desired, &current, &with_enable_drop(true))
        .expect("diff should succeed");

    assert_eq!(ops, vec![DiffOp::CreateTable(desired_table)]);
}

#[test]
fn drops_table_when_missing_in_desired_and_enable_drop_is_true() {
    let engine = DiffEngine::new();
    let current = vec![SchemaObject::Table(base_table("users"))];

    let ops = engine
        .diff(&[], &current, &with_enable_drop(true))
        .expect("diff should succeed");

    assert_eq!(ops, vec![DiffOp::DropTable(qualified("users"))]);
}

#[test]
fn emits_alter_column_for_type_default_and_not_null_changes() {
    let engine = DiffEngine::new();

    let mut current_table = base_table("users");
    current_table
        .columns
        .push(column("age", DataType::Integer, false, None));

    let mut desired_table = base_table("users");
    desired_table.columns.push(column(
        "age",
        DataType::BigInt,
        true,
        Some(Expr::Literal(Literal::Integer(0))),
    ));

    let ops = engine
        .diff(
            &[SchemaObject::Table(desired_table)],
            &[SchemaObject::Table(current_table)],
            &with_enable_drop(true),
        )
        .expect("diff should succeed");

    assert_eq!(ops.len(), 1);
    assert_eq!(
        ops[0],
        DiffOp::AlterColumn {
            table: qualified("users"),
            column: ident("age"),
            changes: vec![
                ColumnChange::SetType(DataType::BigInt),
                ColumnChange::SetNotNull(true),
                ColumnChange::SetDefault(Some(Expr::Literal(Literal::Integer(0)))),
            ],
        }
    );
}

#[test]
fn emits_add_and_drop_index_for_table_index_differences() {
    let engine = DiffEngine::new();

    let desired_index = table_index("users", "users_email_idx");
    let dropped_index_name = ident("users_name_idx");
    let current_index = table_index("users", &dropped_index_name.value);

    let desired = vec![
        SchemaObject::Table(base_table("users")),
        SchemaObject::Index(desired_index.clone()),
    ];
    let current = vec![
        SchemaObject::Table(base_table("users")),
        SchemaObject::Index(current_index),
    ];

    let ops = engine
        .diff(&desired, &current, &with_enable_drop(true))
        .expect("diff should succeed");

    assert_eq!(ops.len(), 2);
    assert!(ops.contains(&DiffOp::AddIndex(desired_index)));
    assert!(ops.contains(&DiffOp::DropIndex {
        owner: IndexOwner::Table(qualified("users")),
        name: dropped_index_name,
    }));
}

#[test]
fn uses_equivalence_policy_for_default_and_check_expression_comparison() {
    let engine = DiffEngine::new();

    let mut current_table = base_table("users");
    current_table.columns.push(column(
        "quantity",
        DataType::Integer,
        false,
        Some(Expr::Raw("'0'::integer".to_string())),
    ));
    current_table.checks.push(CheckConstraint {
        name: Some(ident("users_quantity_check")),
        expr: Expr::Raw("quantity > '0'::integer".to_string()),
        no_inherit: false,
    });

    let mut desired_table = base_table("users");
    desired_table.columns.push(column(
        "quantity",
        DataType::Integer,
        false,
        Some(Expr::Literal(Literal::Integer(0))),
    ));
    desired_table.checks.push(CheckConstraint {
        name: Some(ident("users_quantity_check")),
        expr: Expr::Raw("quantity > 0".to_string()),
        no_inherit: false,
    });

    let strict_ops = engine
        .diff(
            &[SchemaObject::Table(desired_table.clone())],
            &[SchemaObject::Table(current_table.clone())],
            &with_enable_drop(true),
        )
        .expect("strict comparison should succeed");
    assert!(
        !strict_ops.is_empty(),
        "strict comparison must treat different expressions as changes",
    );

    let relaxed_config = DiffConfig::new(false, Vec::new(), Arc::new(CastLiteralExprPolicy));
    let relaxed_ops = engine
        .diff(
            &[SchemaObject::Table(desired_table)],
            &[SchemaObject::Table(current_table)],
            &relaxed_config,
        )
        .expect("relaxed comparison should succeed");
    assert!(
        relaxed_ops.is_empty(),
        "policy should suppress diff for semantically equivalent expressions",
    );
}

#[test]
fn fails_fast_when_index_owner_is_missing() {
    let engine = DiffEngine::new();
    let desired = vec![SchemaObject::Index(table_index(
        "missing_table",
        "idx_missing",
    ))];

    let error = engine
        .diff(&desired, &[], &with_enable_drop(true))
        .expect_err("missing index owner must fail fast");

    match error {
        Error::Diff(DiffError::ObjectComparison { target, operation }) => {
            assert!(target.contains("missing_table"));
            assert!(operation.contains("owner"));
        }
        other => panic!("unexpected error: {other}"),
    }
}
