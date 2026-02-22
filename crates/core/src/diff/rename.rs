use std::collections::{BTreeMap, BTreeSet};

use crate::{Ident, IndexDef, Value};

const INDEX_RENAMED_FROM_EXTRA_KEY: &str = "stateql.renamed_from";

pub(super) fn resolve_rename_match<'a, K, V>(
    renamed_from: Option<&K>,
    current_by_key: &'a BTreeMap<K, V>,
    matched_current_keys: &BTreeSet<K>,
) -> Option<(&'a K, &'a V)>
where
    K: Ord,
{
    let from_key = renamed_from?;
    if matched_current_keys.contains(from_key) {
        return None;
    }

    current_by_key.get_key_value(from_key)
}

pub(super) fn index_renamed_from(index: &IndexDef) -> Option<Ident> {
    let Value::String(name) = index.extra.get(INDEX_RENAMED_FROM_EXTRA_KEY)? else {
        return None;
    };

    Some(Ident::unquoted(name))
}

pub(super) fn indexes_equivalent_for_rename(desired: &IndexDef, current: &IndexDef) -> bool {
    let mut desired_normalized = desired.clone();
    desired_normalized.name = current.name.clone();
    desired_normalized
        .extra
        .remove(INDEX_RENAMED_FROM_EXTRA_KEY);

    let mut current_normalized = current.clone();
    current_normalized
        .extra
        .remove(INDEX_RENAMED_FROM_EXTRA_KEY);

    desired_normalized == current_normalized
}
