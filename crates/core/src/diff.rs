pub mod compare;
mod compare_remaining;
mod constraint_pairing;
mod cycle;
mod enable_drop;
pub mod engine;
mod name_resolution;
mod partition;
pub mod policy;
mod privilege;
mod rename;
pub mod types;
mod view_rebuild;

pub use compare::DiffEngine;
pub use enable_drop::{DiffDiagnostics, DiffOutcome, SkippedOpDiagnostic, SkippedOpKind};
pub use policy::{
    DEFAULT_EQUIVALENCE_POLICY, DefaultEquivalencePolicy, DiffConfig, EquivalencePolicy,
    EquivalencePolicyContractError, custom_types_equivalent, exprs_equivalent,
    verify_equivalence_policy_contract,
};
pub use types::{
    ColumnChange, DiffOp, DomainChange, SequenceChange, TypeChange,
    is_mysql_change_column_full_redefinition,
};
