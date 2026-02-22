use std::collections::{BTreeMap, BTreeSet};

use crate::{DiffOp, Ident, QualifiedName, Table, View};

#[derive(Debug, Clone)]
struct IndexedOp {
    original_index: usize,
    op: DiffOp,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum PriorityGroup {
    DropPolicy = 1,
    DropTrigger = 2,
    DropView = 3,
    DropForeignKey = 4,
    DropIndex = 5,
    DropTable = 6,
    DropSequence = 7,
    DropDomain = 8,
    DropType = 9,
    DropFunction = 10,
    DropSchema = 11,
    DropExtension = 12,
    CreateExtension = 13,
    CreateSchema = 14,
    CreateType = 15,
    AlterType = 16,
    CreateDomain = 17,
    AlterDomain = 18,
    CreateSequence = 19,
    AlterSequence = 20,
    CreateTable = 21,
    TableScoped = 22,
    AddForeignKey = 23,
    CreateView = 24,
    CreateMaterializedView = 25,
    AddIndex = 26,
    CreateTriggerOrFunction = 27,
    CreatePolicy = 28,
    Comment = 29,
    Privilege = 30,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum TableSubPriority {
    RenameTable = 0,
    RenameColumn = 1,
    AlterColumn = 2,
    AddColumn = 3,
    DropColumn = 4,
    PrimaryKey = 5,
    Constraints = 6,
    Partition = 7,
    TableOptions = 8,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct IdentKey {
    value: String,
    quoted: bool,
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
struct QualifiedNameKey {
    schema: Option<IdentKey>,
    name: IdentKey,
}

impl From<&QualifiedName> for QualifiedNameKey {
    fn from(value: &QualifiedName) -> Self {
        Self {
            schema: value.schema.as_ref().map(IdentKey::from),
            name: IdentKey::from(&value.name),
        }
    }
}

#[derive(Debug, Clone)]
struct ViewReference {
    schema: Option<Ident>,
    name: Ident,
}

#[must_use]
pub fn sort_diff_ops(ops: Vec<DiffOp>) -> Vec<DiffOp> {
    let mut grouped = BTreeMap::<PriorityGroup, Vec<IndexedOp>>::new();
    for (original_index, op) in ops.into_iter().enumerate() {
        grouped
            .entry(priority_group(&op))
            .or_default()
            .push(IndexedOp { original_index, op });
    }

    let mut sorted = Vec::new();
    for (priority, entries) in grouped {
        let mut prioritized = match priority {
            PriorityGroup::CreateTable => sort_create_tables(entries),
            PriorityGroup::CreateView => sort_create_views(entries),
            PriorityGroup::TableScoped => sort_table_scoped(entries),
            _ => entries,
        };
        sorted.extend(prioritized.drain(..).map(|entry| entry.op));
    }

    sorted
}

fn priority_group(op: &DiffOp) -> PriorityGroup {
    match op {
        DiffOp::DropPolicy { .. } => PriorityGroup::DropPolicy,
        DiffOp::DropTrigger { .. } => PriorityGroup::DropTrigger,
        DiffOp::DropView(_) | DiffOp::DropMaterializedView(_) => PriorityGroup::DropView,
        DiffOp::DropForeignKey { .. } => PriorityGroup::DropForeignKey,
        DiffOp::DropIndex { .. } => PriorityGroup::DropIndex,
        DiffOp::DropTable(_) => PriorityGroup::DropTable,
        DiffOp::DropSequence(_) => PriorityGroup::DropSequence,
        DiffOp::DropDomain(_) => PriorityGroup::DropDomain,
        DiffOp::DropType(_) => PriorityGroup::DropType,
        DiffOp::DropFunction(_) => PriorityGroup::DropFunction,
        DiffOp::DropSchema(_) => PriorityGroup::DropSchema,
        DiffOp::DropExtension(_) => PriorityGroup::DropExtension,
        DiffOp::CreateExtension(_) => PriorityGroup::CreateExtension,
        DiffOp::CreateSchema(_) => PriorityGroup::CreateSchema,
        DiffOp::CreateType(_) => PriorityGroup::CreateType,
        DiffOp::AlterType { .. } => PriorityGroup::AlterType,
        DiffOp::CreateDomain(_) => PriorityGroup::CreateDomain,
        DiffOp::AlterDomain { .. } => PriorityGroup::AlterDomain,
        DiffOp::CreateSequence(_) => PriorityGroup::CreateSequence,
        DiffOp::AlterSequence { .. } => PriorityGroup::AlterSequence,
        DiffOp::CreateTable(_) => PriorityGroup::CreateTable,
        DiffOp::RenameTable { .. }
        | DiffOp::RenameColumn { .. }
        | DiffOp::AlterColumn { .. }
        | DiffOp::AddColumn { .. }
        | DiffOp::DropColumn { .. }
        | DiffOp::SetPrimaryKey { .. }
        | DiffOp::DropPrimaryKey { .. }
        | DiffOp::AddCheck { .. }
        | DiffOp::DropCheck { .. }
        | DiffOp::AddExclusion { .. }
        | DiffOp::DropExclusion { .. }
        | DiffOp::AddPartition { .. }
        | DiffOp::DropPartition { .. }
        | DiffOp::AlterTableOptions { .. } => PriorityGroup::TableScoped,
        DiffOp::AddForeignKey { .. } => PriorityGroup::AddForeignKey,
        DiffOp::CreateView(_) => PriorityGroup::CreateView,
        DiffOp::CreateMaterializedView(_) => PriorityGroup::CreateMaterializedView,
        DiffOp::AddIndex(_) | DiffOp::RenameIndex { .. } => PriorityGroup::AddIndex,
        DiffOp::CreateTrigger(_) | DiffOp::CreateFunction(_) => {
            PriorityGroup::CreateTriggerOrFunction
        }
        DiffOp::CreatePolicy(_) => PriorityGroup::CreatePolicy,
        DiffOp::SetComment(_) | DiffOp::DropComment { .. } => PriorityGroup::Comment,
        DiffOp::Grant(_) | DiffOp::Revoke(_) => PriorityGroup::Privilege,
    }
}

fn sort_table_scoped(mut entries: Vec<IndexedOp>) -> Vec<IndexedOp> {
    let mut table_order = BTreeMap::<QualifiedNameKey, usize>::new();
    let mut next_table_order = 0usize;

    for entry in &entries {
        if let Some(table_key) = table_key_for_table_scoped_op(&entry.op)
            && !table_order.contains_key(&table_key)
        {
            table_order.insert(table_key, next_table_order);
            next_table_order += 1;
        }
    }

    entries.sort_by_key(|entry| {
        let table_rank = table_key_for_table_scoped_op(&entry.op)
            .and_then(|table_key| table_order.get(&table_key).copied())
            .unwrap_or(usize::MAX);
        (
            table_rank,
            table_sub_priority(&entry.op),
            entry.original_index,
        )
    });
    entries
}

fn table_key_for_table_scoped_op(op: &DiffOp) -> Option<QualifiedNameKey> {
    match op {
        DiffOp::RenameTable { to, .. } => Some(QualifiedNameKey::from(to)),
        DiffOp::RenameColumn { table, .. }
        | DiffOp::AlterColumn { table, .. }
        | DiffOp::AddColumn { table, .. }
        | DiffOp::DropColumn { table, .. }
        | DiffOp::SetPrimaryKey { table, .. }
        | DiffOp::DropPrimaryKey { table }
        | DiffOp::AddCheck { table, .. }
        | DiffOp::DropCheck { table, .. }
        | DiffOp::AddExclusion { table, .. }
        | DiffOp::DropExclusion { table, .. }
        | DiffOp::AddPartition { table, .. }
        | DiffOp::DropPartition { table, .. }
        | DiffOp::AlterTableOptions { table, .. } => Some(QualifiedNameKey::from(table)),
        _ => None,
    }
}

fn table_sub_priority(op: &DiffOp) -> TableSubPriority {
    match op {
        DiffOp::RenameTable { .. } => TableSubPriority::RenameTable,
        DiffOp::RenameColumn { .. } => TableSubPriority::RenameColumn,
        DiffOp::AlterColumn { .. } => TableSubPriority::AlterColumn,
        DiffOp::AddColumn { .. } => TableSubPriority::AddColumn,
        DiffOp::DropColumn { .. } => TableSubPriority::DropColumn,
        DiffOp::SetPrimaryKey { .. } | DiffOp::DropPrimaryKey { .. } => {
            TableSubPriority::PrimaryKey
        }
        DiffOp::AddCheck { .. }
        | DiffOp::DropCheck { .. }
        | DiffOp::AddExclusion { .. }
        | DiffOp::DropExclusion { .. } => TableSubPriority::Constraints,
        DiffOp::AddPartition { .. } | DiffOp::DropPartition { .. } => TableSubPriority::Partition,
        DiffOp::AlterTableOptions { .. } => TableSubPriority::TableOptions,
        _ => TableSubPriority::TableOptions,
    }
}

fn sort_create_tables(entries: Vec<IndexedOp>) -> Vec<IndexedOp> {
    let mut index_by_table = BTreeMap::<QualifiedNameKey, usize>::new();
    for (idx, entry) in entries.iter().enumerate() {
        if let DiffOp::CreateTable(table) = &entry.op {
            index_by_table.insert(QualifiedNameKey::from(&table.name), idx);
        }
    }

    let mut dependencies = vec![BTreeSet::<usize>::new(); entries.len()];
    for (idx, entry) in entries.iter().enumerate() {
        let DiffOp::CreateTable(table) = &entry.op else {
            continue;
        };
        add_table_dependencies(idx, table, &index_by_table, &mut dependencies);
    }

    topological_sort(entries, dependencies)
}

fn add_table_dependencies(
    idx: usize,
    table: &Table,
    index_by_table: &BTreeMap<QualifiedNameKey, usize>,
    dependencies: &mut [BTreeSet<usize>],
) {
    let self_key = QualifiedNameKey::from(&table.name);
    for foreign_key in &table.foreign_keys {
        let dependency_key = QualifiedNameKey::from(&foreign_key.referenced_table);
        if dependency_key == self_key {
            continue;
        }

        if let Some(dependency_index) = index_by_table.get(&dependency_key) {
            dependencies[idx].insert(*dependency_index);
        }
    }
}

fn sort_create_views(entries: Vec<IndexedOp>) -> Vec<IndexedOp> {
    let mut index_by_view = BTreeMap::<QualifiedNameKey, usize>::new();
    for (idx, entry) in entries.iter().enumerate() {
        if let DiffOp::CreateView(view) = &entry.op {
            index_by_view.insert(QualifiedNameKey::from(&view.name), idx);
        }
    }

    let mut dependencies = vec![BTreeSet::<usize>::new(); entries.len()];
    for (idx, entry) in entries.iter().enumerate() {
        let DiffOp::CreateView(view) = &entry.op else {
            continue;
        };
        add_view_dependencies(idx, view, &index_by_view, &mut dependencies);
    }

    topological_sort(entries, dependencies)
}

fn add_view_dependencies(
    idx: usize,
    view: &View,
    index_by_view: &BTreeMap<QualifiedNameKey, usize>,
    dependencies: &mut [BTreeSet<usize>],
) {
    let self_key = QualifiedNameKey::from(&view.name);
    for reference in extract_relation_references(&view.query) {
        if let Some(dependency_key) = resolve_view_reference(&view.name, &reference, index_by_view)
            && dependency_key != self_key
            && let Some(dependency_index) = index_by_view.get(&dependency_key)
        {
            dependencies[idx].insert(*dependency_index);
        }
    }
}

fn topological_sort(entries: Vec<IndexedOp>, dependencies: Vec<BTreeSet<usize>>) -> Vec<IndexedOp> {
    let mut reverse_edges = vec![BTreeSet::<usize>::new(); entries.len()];
    for (idx, deps) in dependencies.iter().enumerate() {
        for dependency in deps {
            reverse_edges[*dependency].insert(idx);
        }
    }

    let mut remaining_dependencies = dependencies.iter().map(BTreeSet::len).collect::<Vec<_>>();
    let mut ready = BTreeSet::<(usize, usize)>::new();
    for (idx, count) in remaining_dependencies.iter().enumerate() {
        if *count == 0 {
            ready.insert((entries[idx].original_index, idx));
        }
    }

    let mut ordered_indexes = Vec::new();
    let mut visited = vec![false; entries.len()];
    while let Some((_, idx)) = ready.pop_first() {
        if visited[idx] {
            continue;
        }
        visited[idx] = true;
        ordered_indexes.push(idx);

        for dependent in &reverse_edges[idx] {
            if remaining_dependencies[*dependent] == 0 {
                continue;
            }
            remaining_dependencies[*dependent] -= 1;
            if remaining_dependencies[*dependent] == 0 {
                ready.insert((entries[*dependent].original_index, *dependent));
            }
        }
    }

    if ordered_indexes.len() != entries.len() {
        let mut unresolved = (0..entries.len())
            .filter(|idx| !visited[*idx])
            .collect::<Vec<_>>();
        unresolved.sort_by_key(|idx| entries[*idx].original_index);
        ordered_indexes.extend(unresolved);
    }

    ordered_indexes
        .into_iter()
        .map(|idx| entries[idx].clone())
        .collect()
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
    index_by_view: &BTreeMap<QualifiedNameKey, usize>,
) -> Option<QualifiedNameKey> {
    if let Some(schema) = &reference.schema {
        let qualified = QualifiedNameKey {
            schema: Some(IdentKey::from(schema)),
            name: IdentKey::from(&reference.name),
        };
        if index_by_view.contains_key(&qualified) {
            return Some(qualified);
        }
    }

    if let Some(source_schema) = &source_view.schema {
        let schema_local = QualifiedNameKey {
            schema: Some(IdentKey::from(source_schema)),
            name: IdentKey::from(&reference.name),
        };
        if index_by_view.contains_key(&schema_local) {
            return Some(schema_local);
        }
    }

    let unqualified = QualifiedNameKey {
        schema: None,
        name: IdentKey::from(&reference.name),
    };
    if index_by_view.contains_key(&unqualified) {
        return Some(unqualified);
    }

    let mut matching_names = index_by_view
        .keys()
        .filter(|candidate| candidate.name == unqualified.name)
        .cloned()
        .collect::<Vec<_>>();
    if matching_names.len() == 1 {
        return matching_names.pop();
    }

    None
}
