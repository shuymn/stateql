use std::collections::{BTreeMap, BTreeSet};

use crate::{Ident, IndexOwner, QualifiedName};

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub(super) struct IdentKey {
    value: String,
    quoted: bool,
}

impl IdentKey {
    fn unquoted(value: &str) -> Self {
        Self {
            value: value.to_string(),
            quoted: false,
        }
    }
}

impl From<&Ident> for IdentKey {
    fn from(value: &Ident) -> Self {
        Self {
            value: value.value.clone(),
            quoted: value.quoted,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub(super) struct QualifiedNameKey {
    pub(super) schema: Option<IdentKey>,
    pub(super) name: IdentKey,
}

impl QualifiedNameKey {
    fn with_schema(name: IdentKey, schema: &str) -> Self {
        Self {
            schema: Some(IdentKey::unquoted(schema)),
            name,
        }
    }

    fn without_schema(name: IdentKey) -> Self {
        Self { schema: None, name }
    }
}

impl From<&QualifiedName> for QualifiedNameKey {
    fn from(value: &QualifiedName) -> Self {
        Self {
            schema: value.schema.as_ref().map(IdentKey::from),
            name: IdentKey::from(&value.name),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub(super) enum IndexOwnerKey {
    Table(QualifiedNameKey),
    View(QualifiedNameKey),
    MaterializedView(QualifiedNameKey),
}

impl IndexOwnerKey {
    fn qualified_candidates(&self, schema_search_path: &[String]) -> Vec<Self> {
        let Some(name) = self.unqualified_name().map(|key| key.name.clone()) else {
            return Vec::new();
        };

        schema_search_path
            .iter()
            .map(|schema| {
                self.rebuild_with_name(QualifiedNameKey::with_schema(name.clone(), schema))
            })
            .collect()
    }

    fn unqualified_candidate(&self, schema_search_path: &[String]) -> Option<Self> {
        let name = self.qualified_name()?;
        if !schema_in_search_path(name.schema.as_ref()?, schema_search_path) {
            return None;
        }

        Some(self.rebuild_with_name(QualifiedNameKey::without_schema(name.name.clone())))
    }

    fn qualified_name(&self) -> Option<&QualifiedNameKey> {
        match self {
            Self::Table(name) | Self::View(name) | Self::MaterializedView(name) => {
                if name.schema.is_some() {
                    Some(name)
                } else {
                    None
                }
            }
        }
    }

    fn unqualified_name(&self) -> Option<&QualifiedNameKey> {
        match self {
            Self::Table(name) | Self::View(name) | Self::MaterializedView(name) => {
                if name.schema.is_none() {
                    Some(name)
                } else {
                    None
                }
            }
        }
    }

    fn rebuild_with_name(&self, name: QualifiedNameKey) -> Self {
        match self {
            Self::Table(_) => Self::Table(name),
            Self::View(_) => Self::View(name),
            Self::MaterializedView(_) => Self::MaterializedView(name),
        }
    }
}

impl From<&IndexOwner> for IndexOwnerKey {
    fn from(value: &IndexOwner) -> Self {
        match value {
            IndexOwner::Table(name) => Self::Table(QualifiedNameKey::from(name)),
            IndexOwner::View(name) => Self::View(QualifiedNameKey::from(name)),
            IndexOwner::MaterializedView(name) => {
                Self::MaterializedView(QualifiedNameKey::from(name))
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub(super) struct IndexLookupKey {
    pub(super) owner: IndexOwnerKey,
    pub(super) name: IdentKey,
}

pub(super) fn resolve_qualified_name_match<'a, V>(
    desired_key: &QualifiedNameKey,
    current_by_key: &'a BTreeMap<QualifiedNameKey, V>,
    matched_current_keys: &BTreeSet<QualifiedNameKey>,
    schema_search_path: &[String],
) -> Option<(&'a QualifiedNameKey, &'a V)> {
    if desired_key.schema.is_none() {
        for schema in schema_search_path {
            let candidate = QualifiedNameKey::with_schema(desired_key.name.clone(), schema);
            if matched_current_keys.contains(&candidate) {
                continue;
            }
            if let Some((key, value)) = current_by_key.get_key_value(&candidate) {
                return Some((key, value));
            }
        }
        return None;
    }

    if !schema_in_search_path(desired_key.schema.as_ref()?, schema_search_path) {
        return None;
    }

    let candidate = QualifiedNameKey::without_schema(desired_key.name.clone());
    if matched_current_keys.contains(&candidate) {
        return None;
    }

    current_by_key.get_key_value(&candidate)
}

pub(super) fn resolve_index_match<'a, V>(
    desired_key: &IndexLookupKey,
    current_by_key: &'a BTreeMap<IndexLookupKey, V>,
    matched_current_keys: &BTreeSet<IndexLookupKey>,
    schema_search_path: &[String],
) -> Option<(&'a IndexLookupKey, &'a V)> {
    for candidate_owner in desired_key.owner.qualified_candidates(schema_search_path) {
        let candidate = IndexLookupKey {
            owner: candidate_owner,
            name: desired_key.name.clone(),
        };
        if matched_current_keys.contains(&candidate) {
            continue;
        }
        if let Some((key, value)) = current_by_key.get_key_value(&candidate) {
            return Some((key, value));
        }
    }

    let candidate_owner = desired_key
        .owner
        .unqualified_candidate(schema_search_path)?;
    let candidate = IndexLookupKey {
        owner: candidate_owner,
        name: desired_key.name.clone(),
    };
    if matched_current_keys.contains(&candidate) {
        return None;
    }

    current_by_key.get_key_value(&candidate)
}

fn schema_in_search_path(schema: &IdentKey, schema_search_path: &[String]) -> bool {
    schema_search_path
        .iter()
        .any(|candidate| IdentKey::unquoted(candidate) == *schema)
}
