use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

use stateql_core::{
    DiffConfig, EquivalencePolicy, EquivalencePolicyContractError, Expr, Literal,
    custom_types_equivalent, exprs_equivalent, verify_equivalence_policy_contract,
};

#[derive(Debug)]
struct RelaxedPolicy;

impl EquivalencePolicy for RelaxedPolicy {
    fn is_equivalent_expr(&self, left: &Expr, right: &Expr) -> bool {
        matches!(
            (left, right),
            (Expr::Raw(l), Expr::Literal(Literal::String(r)))
                if l == "'0'::integer" && r == "0"
        ) || matches!(
            (left, right),
            (Expr::Literal(Literal::String(l)), Expr::Raw(r))
                if l == "0" && r == "'0'::integer"
        )
    }

    fn is_equivalent_custom_type(&self, left: &str, right: &str) -> bool {
        matches!((left, right), ("int4", "integer") | ("integer", "int4"))
    }
}

#[derive(Debug)]
struct NonSymmetricPolicy;

impl EquivalencePolicy for NonSymmetricPolicy {
    fn is_equivalent_expr(&self, left: &Expr, right: &Expr) -> bool {
        matches!(left, Expr::Raw(v) if v == "lhs") && matches!(right, Expr::Raw(v) if v == "rhs")
    }
}

#[derive(Debug, Default)]
struct FlakyPolicy {
    next_result: AtomicBool,
}

impl EquivalencePolicy for FlakyPolicy {
    fn is_equivalent_custom_type(&self, _left: &str, _right: &str) -> bool {
        self.next_result.fetch_xor(true, Ordering::SeqCst)
    }
}

#[test]
fn diff_config_uses_injected_policy_for_expr_and_custom_type_relaxation() {
    let config = DiffConfig::new(false, vec![], Arc::new(RelaxedPolicy));
    let left_expr = Expr::Raw("'0'::integer".to_string());
    let right_expr = Expr::Literal(Literal::String("0".to_string()));

    assert!(exprs_equivalent(
        config.equivalence_policy.as_ref(),
        &left_expr,
        &right_expr
    ));
    assert!(custom_types_equivalent(
        config.equivalence_policy.as_ref(),
        "int4",
        "integer"
    ));

    let strict_left = Expr::Raw("users.id".to_string());
    let strict_right = Expr::Raw("accounts.id".to_string());
    assert!(!exprs_equivalent(
        config.equivalence_policy.as_ref(),
        &strict_left,
        &strict_right
    ));
}

#[test]
fn contract_helper_detects_non_symmetric_expr_policy() {
    let left = Expr::Raw("lhs".to_string());
    let right = Expr::Raw("rhs".to_string());
    let err = verify_equivalence_policy_contract(
        &NonSymmetricPolicy,
        &[(&left, &right)],
        &[("int4", "integer")],
    )
    .expect_err("non-symmetric policy should be rejected");

    assert_eq!(
        err,
        EquivalencePolicyContractError::ExprNotSymmetric { case_index: 0 }
    );
}

#[test]
fn contract_helper_detects_non_stable_custom_type_policy() {
    let left = Expr::Raw("lhs".to_string());
    let right = Expr::Raw("rhs".to_string());
    let err = verify_equivalence_policy_contract(
        &FlakyPolicy::default(),
        &[(&left, &right)],
        &[("int4", "integer")],
    )
    .expect_err("non-stable policy should be rejected");

    assert_eq!(
        err,
        EquivalencePolicyContractError::CustomTypeNotStable { case_index: 0 }
    );
}
