use std::{collections::BTreeMap, sync::Arc};

use stateql_core::{
    CheckConstraint, Column, DataType, Dialect, DiffConfig, DiffEngine, EquivalencePolicy, Expr,
    Ident, Literal, SchemaObject, Table, verify_equivalence_policy_contract,
};
use stateql_dialect_sqlite::SqliteDialect;

#[derive(Clone, Copy)]
struct StaticPolicyRef {
    inner: &'static dyn EquivalencePolicy,
}

impl EquivalencePolicy for StaticPolicyRef {
    fn is_equivalent_expr(&self, left: &Expr, right: &Expr) -> bool {
        self.inner.is_equivalent_expr(left, right)
    }

    fn is_equivalent_custom_type(&self, left: &str, right: &str) -> bool {
        self.inner.is_equivalent_custom_type(left, right)
    }
}

#[test]
fn strict_diff_reports_cast_alias_residual_but_sqlite_policy_suppresses_it() {
    let dialect = SqliteDialect;
    let mut desired = vec![quantity_table(
        Expr::Raw("0".to_string()),
        Expr::Raw("quantity > 0".to_string()),
    )];
    let mut current = vec![quantity_table(
        Expr::Raw("CAST('000' AS INT)".to_string()),
        Expr::Raw("quantity > 0".to_string()),
    )];

    normalize_all(&dialect, &mut desired);
    normalize_all(&dialect, &mut current);

    let strict_ops = diff_with_config(&desired, &current, &DiffConfig::default());
    assert!(
        !strict_ops.is_empty(),
        "default policy should still produce false diffs for cast/alias residuals",
    );

    let relaxed_ops = diff_with_config(&desired, &current, &sqlite_diff_config(&dialect));
    assert!(
        relaxed_ops.is_empty(),
        "sqlite equivalence policy should suppress cast/alias residual diffs",
    );
}

#[test]
fn strict_diff_reports_paren_and_whitespace_residuals_but_sqlite_policy_suppresses_them() {
    let dialect = SqliteDialect;
    let mut desired = vec![quantity_table(
        Expr::Raw("((0))".to_string()),
        Expr::Raw("quantity    >     0".to_string()),
    )];
    let mut current = vec![quantity_table(
        Expr::Raw("0".to_string()),
        Expr::Raw("( quantity > 0 )".to_string()),
    )];

    normalize_all(&dialect, &mut desired);
    normalize_all(&dialect, &mut current);

    let strict_ops = diff_with_config(&desired, &current, &DiffConfig::default());
    assert!(
        !strict_ops.is_empty(),
        "default policy should still produce false diffs for paren/whitespace residuals",
    );

    let relaxed_ops = diff_with_config(&desired, &current, &sqlite_diff_config(&dialect));
    assert!(
        relaxed_ops.is_empty(),
        "sqlite equivalence policy should suppress paren/whitespace residual diffs",
    );
}

#[test]
fn sqlite_policy_contract_is_symmetric_and_stable_and_keeps_structural_mismatch_strict() {
    let dialect = SqliteDialect;
    let policy = dialect.equivalence_policy();

    let cast_left = Expr::Raw("CAST('000' AS INT)".to_string());
    let cast_right = Expr::Raw("0".to_string());
    let paren_left = Expr::Raw("((quantity > 0))".to_string());
    let paren_right = Expr::Raw("quantity > 0".to_string());
    let whitespace_left = Expr::Raw("quantity    >     0".to_string());
    let whitespace_right = Expr::Raw("quantity > 0".to_string());

    verify_equivalence_policy_contract(
        policy,
        &[
            (&cast_left, &cast_right),
            (&paren_left, &paren_right),
            (&whitespace_left, &whitespace_right),
        ],
        &[],
    )
    .expect("sqlite equivalence policy must satisfy symmetry/stability contract");

    assert!(policy.is_equivalent_expr(&cast_left, &cast_right));
    assert!(policy.is_equivalent_expr(&paren_left, &paren_right));
    assert!(policy.is_equivalent_expr(&whitespace_left, &whitespace_right));

    let literal_zero = Expr::Literal(Literal::Integer(0));
    let raw_zero = Expr::Raw("0".to_string());
    assert!(
        !policy.is_equivalent_expr(&literal_zero, &raw_zero),
        "policy must not relax structural mismatches"
    );
}

fn normalize_all(dialect: &SqliteDialect, objects: &mut [SchemaObject]) {
    for object in objects {
        dialect.normalize(object);
    }
}

fn diff_with_config(
    desired: &[SchemaObject],
    current: &[SchemaObject],
    config: &DiffConfig,
) -> Vec<stateql_core::DiffOp> {
    DiffEngine::new()
        .diff(desired, current, config)
        .expect("diff should succeed")
}

fn sqlite_diff_config(dialect: &SqliteDialect) -> DiffConfig {
    DiffConfig::new(
        false,
        Vec::new(),
        Arc::new(StaticPolicyRef {
            inner: dialect.equivalence_policy(),
        }),
    )
}

fn quantity_table(default_expr: Expr, check_expr: Expr) -> SchemaObject {
    let mut table = Table::named("users");
    table.columns.push(Column {
        name: Ident::unquoted("quantity"),
        data_type: DataType::Integer,
        not_null: false,
        default: Some(default_expr),
        identity: None,
        generated: None,
        comment: None,
        collation: None,
        renamed_from: None,
        extra: BTreeMap::new(),
    });
    table.checks.push(CheckConstraint {
        name: Some(Ident::unquoted("users_quantity_check")),
        expr: check_expr,
        no_inherit: false,
    });
    SchemaObject::Table(table)
}
