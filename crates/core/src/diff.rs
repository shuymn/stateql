pub mod compare;
mod compare_remaining;
pub mod engine;
mod name_resolution;
mod partition;
pub mod policy;
mod rename;
pub mod types;

pub use compare::DiffEngine;
pub use policy::{
    DEFAULT_EQUIVALENCE_POLICY, DefaultEquivalencePolicy, DiffConfig, EquivalencePolicy,
    EquivalencePolicyContractError, custom_types_equivalent, exprs_equivalent,
    verify_equivalence_policy_contract,
};
pub use types::{
    ColumnChange, DiffOp, DomainChange, SequenceChange, TypeChange,
    is_mysql_change_column_full_redefinition,
};
