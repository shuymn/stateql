use crate::{DiffConfig, DiffOp, Privilege, PrivilegeOp};

const ORDERED_PRIVILEGE_OPS: [PrivilegeOp; 13] = [
    PrivilegeOp::Select,
    PrivilegeOp::Insert,
    PrivilegeOp::Update,
    PrivilegeOp::Delete,
    PrivilegeOp::Truncate,
    PrivilegeOp::References,
    PrivilegeOp::Trigger,
    PrivilegeOp::Usage,
    PrivilegeOp::Create,
    PrivilegeOp::Connect,
    PrivilegeOp::Temporary,
    PrivilegeOp::Execute,
    PrivilegeOp::All,
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PrivilegeOpsDiff {
    pub(crate) added: Vec<PrivilegeOp>,
    pub(crate) removed: Vec<PrivilegeOp>,
    pub(crate) shared: Vec<PrivilegeOp>,
}

pub(crate) fn compare_privileges(
    desired: &[&Privilege],
    current: &[&Privilege],
    config: &DiffConfig,
    ops: &mut Vec<DiffOp>,
) {
    let mut matched_current = vec![false; current.len()];

    for desired_privilege in desired {
        if let Some((matched_index, current_privilege)) =
            find_matching_current(desired_privilege, current, &matched_current)
        {
            matched_current[matched_index] = true;
            push_privilege_changes(desired_privilege, current_privilege, config, ops);
            continue;
        }

        let grant_ops = diff_privilege_ops(&desired_privilege.operations, &[]).added;
        push_grant(
            ops,
            desired_privilege,
            grant_ops,
            desired_privilege.with_grant_option,
        );
    }

    if config.enable_drop {
        for (index, current_privilege) in current.iter().enumerate() {
            if matched_current[index] {
                continue;
            }

            let revoke_ops = diff_privilege_ops(&[], &current_privilege.operations).removed;
            push_revoke(ops, current_privilege, revoke_ops, false);
        }
    }
}

pub(crate) fn diff_privilege_ops(
    desired_operations: &[PrivilegeOp],
    current_operations: &[PrivilegeOp],
) -> PrivilegeOpsDiff {
    let desired_set = privilege_op_set(desired_operations);
    let current_set = privilege_op_set(current_operations);

    let mut added = Vec::new();
    let mut removed = Vec::new();
    let mut shared = Vec::new();

    for (index, operation) in ORDERED_PRIVILEGE_OPS.iter().copied().enumerate() {
        match (desired_set[index], current_set[index]) {
            (true, false) => added.push(operation),
            (false, true) => removed.push(operation),
            (true, true) => shared.push(operation),
            (false, false) => {}
        }
    }

    PrivilegeOpsDiff {
        added,
        removed,
        shared,
    }
}

fn push_privilege_changes(
    desired: &Privilege,
    current: &Privilege,
    config: &DiffConfig,
    ops: &mut Vec<DiffOp>,
) {
    let op_diff = diff_privilege_ops(&desired.operations, &current.operations);

    if config.enable_drop {
        push_revoke(ops, current, op_diff.removed, false);
    }

    push_grant(ops, desired, op_diff.added, desired.with_grant_option);

    if !op_diff.shared.is_empty() && desired.with_grant_option != current.with_grant_option {
        if desired.with_grant_option {
            push_grant(ops, desired, op_diff.shared, true);
        } else if config.enable_drop {
            push_revoke(ops, current, op_diff.shared, true);
        }
    }
}

fn find_matching_current<'a>(
    desired: &Privilege,
    current: &'a [&Privilege],
    matched_current: &[bool],
) -> Option<(usize, &'a Privilege)> {
    for (index, current_privilege) in current.iter().enumerate() {
        if matched_current[index] {
            continue;
        }

        if privilege_key_matches(desired, current_privilege) {
            return Some((index, *current_privilege));
        }
    }

    None
}

fn privilege_key_matches(left: &Privilege, right: &Privilege) -> bool {
    left.on == right.on && left.grantee == right.grantee
}

fn push_grant(
    ops: &mut Vec<DiffOp>,
    base: &Privilege,
    operations: Vec<PrivilegeOp>,
    with_grant_option: bool,
) {
    if operations.is_empty() {
        return;
    }

    ops.push(DiffOp::Grant(Privilege {
        operations,
        on: base.on.clone(),
        grantee: base.grantee.clone(),
        with_grant_option,
    }));
}

fn push_revoke(
    ops: &mut Vec<DiffOp>,
    base: &Privilege,
    operations: Vec<PrivilegeOp>,
    with_grant_option: bool,
) {
    if operations.is_empty() {
        return;
    }

    ops.push(DiffOp::Revoke(Privilege {
        operations,
        on: base.on.clone(),
        grantee: base.grantee.clone(),
        with_grant_option,
    }));
}

fn privilege_op_set(operations: &[PrivilegeOp]) -> [bool; ORDERED_PRIVILEGE_OPS.len()] {
    let mut set = [false; ORDERED_PRIVILEGE_OPS.len()];
    for operation in operations {
        set[privilege_op_index(*operation)] = true;
    }

    set
}

const fn privilege_op_index(operation: PrivilegeOp) -> usize {
    match operation {
        PrivilegeOp::Select => 0,
        PrivilegeOp::Insert => 1,
        PrivilegeOp::Update => 2,
        PrivilegeOp::Delete => 3,
        PrivilegeOp::Truncate => 4,
        PrivilegeOp::References => 5,
        PrivilegeOp::Trigger => 6,
        PrivilegeOp::Usage => 7,
        PrivilegeOp::Create => 8,
        PrivilegeOp::Connect => 9,
        PrivilegeOp::Temporary => 10,
        PrivilegeOp::Execute => 11,
        PrivilegeOp::All => 12,
    }
}
