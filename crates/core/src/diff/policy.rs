use std::sync::Arc;

use crate::Expr;

pub trait EquivalencePolicy: Send + Sync {
    fn is_equivalent_expr(&self, left: &Expr, right: &Expr) -> bool {
        left == right
    }

    fn is_equivalent_custom_type(&self, left: &str, right: &str) -> bool {
        left == right
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct DefaultEquivalencePolicy;

impl EquivalencePolicy for DefaultEquivalencePolicy {}

pub static DEFAULT_EQUIVALENCE_POLICY: DefaultEquivalencePolicy = DefaultEquivalencePolicy;

#[derive(Clone)]
pub struct DiffConfig {
    pub enable_drop: bool,
    pub schema_search_path: Vec<String>,
    pub equivalence_policy: Arc<dyn EquivalencePolicy>,
}

impl DiffConfig {
    pub fn new(
        enable_drop: bool,
        schema_search_path: Vec<String>,
        equivalence_policy: Arc<dyn EquivalencePolicy>,
    ) -> Self {
        Self {
            enable_drop,
            schema_search_path,
            equivalence_policy,
        }
    }
}

impl Default for DiffConfig {
    fn default() -> Self {
        Self {
            enable_drop: false,
            schema_search_path: Vec::new(),
            equivalence_policy: Arc::new(DefaultEquivalencePolicy),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EquivalencePolicyContractError {
    ExprNotSymmetric { case_index: usize },
    ExprNotStable { case_index: usize },
    CustomTypeNotSymmetric { case_index: usize },
    CustomTypeNotStable { case_index: usize },
}

pub fn verify_equivalence_policy_contract(
    policy: &dyn EquivalencePolicy,
    expr_cases: &[(&Expr, &Expr)],
    custom_type_cases: &[(&str, &str)],
) -> std::result::Result<(), EquivalencePolicyContractError> {
    for (case_index, (left, right)) in expr_cases.iter().enumerate() {
        let forward_first = policy.is_equivalent_expr(left, right);
        let forward_second = policy.is_equivalent_expr(left, right);
        if forward_first != forward_second {
            return Err(EquivalencePolicyContractError::ExprNotStable { case_index });
        }

        let backward_first = policy.is_equivalent_expr(right, left);
        let backward_second = policy.is_equivalent_expr(right, left);
        if backward_first != backward_second {
            return Err(EquivalencePolicyContractError::ExprNotStable { case_index });
        }

        if forward_first != backward_first {
            return Err(EquivalencePolicyContractError::ExprNotSymmetric { case_index });
        }
    }

    for (case_index, (left, right)) in custom_type_cases.iter().enumerate() {
        let forward_first = policy.is_equivalent_custom_type(left, right);
        let forward_second = policy.is_equivalent_custom_type(left, right);
        if forward_first != forward_second {
            return Err(EquivalencePolicyContractError::CustomTypeNotStable { case_index });
        }

        let backward_first = policy.is_equivalent_custom_type(right, left);
        let backward_second = policy.is_equivalent_custom_type(right, left);
        if backward_first != backward_second {
            return Err(EquivalencePolicyContractError::CustomTypeNotStable { case_index });
        }

        if forward_first != backward_first {
            return Err(EquivalencePolicyContractError::CustomTypeNotSymmetric { case_index });
        }
    }

    Ok(())
}

pub fn exprs_equivalent(policy: &dyn EquivalencePolicy, left: &Expr, right: &Expr) -> bool {
    strict_or_policy(left, right, |a, b| policy.is_equivalent_expr(a, b))
}

pub fn custom_types_equivalent(policy: &dyn EquivalencePolicy, left: &str, right: &str) -> bool {
    strict_or_policy(left, right, |a, b| policy.is_equivalent_custom_type(a, b))
}

fn strict_or_policy<T, F>(left: &T, right: &T, policy_check: F) -> bool
where
    T: PartialEq + ?Sized,
    F: FnOnce(&T, &T) -> bool,
{
    left == right || policy_check(left, right)
}
