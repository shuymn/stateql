use std::collections::{BTreeMap, BTreeSet, VecDeque};

use super::name_resolution::QualifiedNameKey;
use crate::{Ident, QualifiedName, View};

#[derive(Debug, Default)]
pub(super) struct ViewRebuildPlan {
    pub(super) rebuild_set: BTreeSet<QualifiedNameKey>,
    pub(super) drop_order: Vec<QualifiedNameKey>,
    pub(super) create_order: Vec<QualifiedNameKey>,
}

pub(super) fn build_view_rebuild_plan(
    desired_by_key: &BTreeMap<QualifiedNameKey, &View>,
    current_by_key: &BTreeMap<QualifiedNameKey, &View>,
) -> ViewRebuildPlan {
    let changed_roots = changed_view_roots(desired_by_key, current_by_key);
    if changed_roots.is_empty() {
        return ViewRebuildPlan::default();
    }

    let current_dependency_graph = build_dependency_graph(current_by_key);
    let rebuild_set = expand_rebuild_closure(&changed_roots, &current_dependency_graph);

    let mut drop_order = topological_order(&rebuild_set, &current_dependency_graph);
    drop_order.reverse();

    let desired_dependency_graph = build_dependency_graph(desired_by_key);
    let create_set = rebuild_set
        .iter()
        .filter(|key| desired_by_key.contains_key(*key))
        .cloned()
        .collect::<BTreeSet<_>>();
    let create_order = topological_order(&create_set, &desired_dependency_graph);

    ViewRebuildPlan {
        rebuild_set,
        drop_order,
        create_order,
    }
}

fn changed_view_roots(
    desired_by_key: &BTreeMap<QualifiedNameKey, &View>,
    current_by_key: &BTreeMap<QualifiedNameKey, &View>,
) -> BTreeSet<QualifiedNameKey> {
    let mut changed_roots = BTreeSet::new();

    for (view_key, desired_view) in desired_by_key {
        if let Some(current_view) = current_by_key.get(view_key)
            && desired_view != current_view
        {
            changed_roots.insert((*view_key).clone());
        }
    }

    changed_roots
}

fn build_dependency_graph(
    views_by_key: &BTreeMap<QualifiedNameKey, &View>,
) -> BTreeMap<QualifiedNameKey, BTreeSet<QualifiedNameKey>> {
    let mut dependency_graph = BTreeMap::new();

    for (view_key, view) in views_by_key {
        let mut dependencies = BTreeSet::new();
        for reference in extract_relation_references(&view.query) {
            if let Some(dependency) = resolve_view_reference(&view.name, &reference, views_by_key)
                && dependency != *view_key
            {
                dependencies.insert(dependency);
            }
        }
        dependency_graph.insert((*view_key).clone(), dependencies);
    }

    dependency_graph
}

fn expand_rebuild_closure(
    changed_roots: &BTreeSet<QualifiedNameKey>,
    dependency_graph: &BTreeMap<QualifiedNameKey, BTreeSet<QualifiedNameKey>>,
) -> BTreeSet<QualifiedNameKey> {
    let reverse_graph = build_reverse_graph(dependency_graph);
    let mut rebuild_set = changed_roots.clone();
    let mut queue = changed_roots.iter().cloned().collect::<VecDeque<_>>();

    while let Some(view_key) = queue.pop_front() {
        if let Some(dependents) = reverse_graph.get(&view_key) {
            for dependent in dependents {
                if rebuild_set.insert(dependent.clone()) {
                    queue.push_back(dependent.clone());
                }
            }
        }
    }

    rebuild_set
}

fn build_reverse_graph(
    dependency_graph: &BTreeMap<QualifiedNameKey, BTreeSet<QualifiedNameKey>>,
) -> BTreeMap<QualifiedNameKey, BTreeSet<QualifiedNameKey>> {
    let mut reverse_graph: BTreeMap<QualifiedNameKey, BTreeSet<QualifiedNameKey>> = BTreeMap::new();

    for (view_key, dependencies) in dependency_graph {
        reverse_graph.entry(view_key.clone()).or_default();

        for dependency in dependencies {
            reverse_graph
                .entry(dependency.clone())
                .or_default()
                .insert(view_key.clone());
        }
    }

    reverse_graph
}

fn topological_order(
    nodes: &BTreeSet<QualifiedNameKey>,
    dependency_graph: &BTreeMap<QualifiedNameKey, BTreeSet<QualifiedNameKey>>,
) -> Vec<QualifiedNameKey> {
    if nodes.is_empty() {
        return Vec::new();
    }

    let mut dependency_count = BTreeMap::new();
    let mut reverse_edges = BTreeMap::new();

    for node in nodes {
        dependency_count.insert(node.clone(), 0usize);
        reverse_edges
            .entry(node.clone())
            .or_insert_with(BTreeSet::new);
    }

    for node in nodes {
        if let Some(dependencies) = dependency_graph.get(node) {
            for dependency in dependencies {
                if !nodes.contains(dependency) {
                    continue;
                }

                if let Some(count) = dependency_count.get_mut(node) {
                    *count += 1;
                }

                reverse_edges
                    .entry(dependency.clone())
                    .or_insert_with(BTreeSet::new)
                    .insert(node.clone());
            }
        }
    }

    let mut ready = dependency_count
        .iter()
        .filter_map(|(node, count)| (*count == 0).then_some(node.clone()))
        .collect::<BTreeSet<_>>();
    let mut ordered = Vec::new();
    let mut visited = BTreeSet::new();

    while let Some(node) = ready.pop_first() {
        if !visited.insert(node.clone()) {
            continue;
        }
        ordered.push(node.clone());

        if let Some(dependents) = reverse_edges.get(&node) {
            for dependent in dependents {
                if let Some(count) = dependency_count.get_mut(dependent) {
                    if *count == 0 {
                        continue;
                    }

                    *count -= 1;
                    if *count == 0 {
                        ready.insert(dependent.clone());
                    }
                }
            }
        }
    }

    if ordered.len() == nodes.len() {
        return ordered;
    }

    for node in nodes {
        if !visited.contains(node) {
            ordered.push(node.clone());
        }
    }

    ordered
}

#[derive(Debug, Clone)]
struct ViewReference {
    schema: Option<Ident>,
    name: Ident,
}

fn extract_relation_references(query: &str) -> Vec<ViewReference> {
    let mut references = Vec::new();
    let mut expect_relation = false;

    for token in query.split_whitespace() {
        if expect_relation {
            if let Some(reference) = parse_relation_token(token) {
                references.push(reference);
                expect_relation = false;
                continue;
            }

            if is_relation_modifier(token) {
                continue;
            }

            expect_relation = false;
            continue;
        }

        if is_relation_keyword(token) {
            expect_relation = true;
        }
    }

    references
}

fn is_relation_keyword(token: &str) -> bool {
    let normalized = normalize_token(token);
    normalized.eq_ignore_ascii_case("from") || normalized.eq_ignore_ascii_case("join")
}

fn is_relation_modifier(token: &str) -> bool {
    let normalized = normalize_token(token);
    normalized.eq_ignore_ascii_case("only") || normalized.eq_ignore_ascii_case("lateral")
}

fn parse_relation_token(token: &str) -> Option<ViewReference> {
    let normalized = normalize_token(token);
    if normalized.is_empty() || normalized.starts_with('(') {
        return None;
    }

    let mut parts = normalized.rsplitn(3, '.');
    let name_part = parts.next()?;
    let schema_part = parts.next();

    let name = parse_ident(name_part)?;
    let schema = schema_part.and_then(parse_ident);

    Some(ViewReference { schema, name })
}

fn normalize_token(token: &str) -> &str {
    token.trim_matches(|ch: char| matches!(ch, ',' | ';' | ')' | '('))
}

fn parse_ident(part: &str) -> Option<Ident> {
    if part.is_empty() {
        return None;
    }

    if let Some(inner) = part
        .strip_prefix('"')
        .and_then(|value| value.strip_suffix('"'))
    {
        let unescaped = inner.replace("\"\"", "\"");
        return Some(Ident::quoted(unescaped));
    }

    Some(Ident::unquoted(part))
}

fn resolve_view_reference(
    source_view: &QualifiedName,
    reference: &ViewReference,
    views_by_key: &BTreeMap<QualifiedNameKey, &View>,
) -> Option<QualifiedNameKey> {
    if let Some(schema) = &reference.schema {
        let qualified = qualified_name_key(Some(schema.clone()), reference.name.clone());
        if views_by_key.contains_key(&qualified) {
            return Some(qualified);
        }
    }

    if let Some(source_schema) = &source_view.schema {
        let schema_local = qualified_name_key(Some(source_schema.clone()), reference.name.clone());
        if views_by_key.contains_key(&schema_local) {
            return Some(schema_local);
        }
    }

    let unqualified = qualified_name_key(None, reference.name.clone());
    if views_by_key.contains_key(&unqualified) {
        return Some(unqualified);
    }

    let mut matching_names = views_by_key
        .keys()
        .filter(|candidate| candidate.name == unqualified.name)
        .cloned()
        .collect::<Vec<_>>();
    if matching_names.len() == 1 {
        return matching_names.pop();
    }

    None
}

fn qualified_name_key(schema: Option<Ident>, name: Ident) -> QualifiedNameKey {
    QualifiedNameKey::from(&QualifiedName { schema, name })
}
