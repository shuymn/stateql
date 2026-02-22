use crate::{CheckConstraint, Ident, QualifiedName};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ConstraintKind {
    Check,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ConstraintKey {
    table: QualifiedName,
    kind: ConstraintKind,
    name: Option<Ident>,
}

impl ConstraintKey {
    fn named_check(table: &QualifiedName, name: &Ident) -> Self {
        Self::new(table, ConstraintKind::Check, Some(name))
    }

    fn new(table: &QualifiedName, kind: ConstraintKind, name: Option<&Ident>) -> Self {
        Self {
            table: table.clone(),
            kind,
            name: name.cloned(),
        }
    }
}

pub(crate) fn check_drop_add_keys_match(
    table: &QualifiedName,
    dropped_name: &Ident,
    added_check: &CheckConstraint,
) -> bool {
    let Some(added_name) = added_check.name.as_ref() else {
        return false;
    };

    let drop_key = ConstraintKey::named_check(table, dropped_name);
    let add_key = ConstraintKey::named_check(table, added_name);
    constraint_keys_match(&drop_key, &add_key)
}

fn constraint_keys_match(left: &ConstraintKey, right: &ConstraintKey) -> bool {
    left == right
}
