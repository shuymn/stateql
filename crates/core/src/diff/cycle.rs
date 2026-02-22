use std::collections::{BTreeMap, BTreeSet};

use crate::{DiffOp, Ident, QualifiedName, Table};

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

#[derive(Debug)]
struct FkDependencyGraph {
    dependencies: Vec<BTreeSet<usize>>,
    index_by_table: BTreeMap<QualifiedNameKey, usize>,
}

pub(super) fn apply_create_cycle_fallback(ops: Vec<DiffOp>) -> Vec<DiffOp> {
    let create_table_positions = ops
        .iter()
        .enumerate()
        .filter_map(|(idx, op)| match op {
            DiffOp::CreateTable(table) => Some((idx, table)),
            _ => None,
        })
        .collect::<Vec<_>>();
    if create_table_positions.len() < 2 {
        return ops;
    }

    let create_tables = create_table_positions
        .iter()
        .map(|(_, table)| *table)
        .collect::<Vec<_>>();
    let graph = build_fk_dependency_graph(&create_tables);
    let cyclic_edges = find_cyclic_edges(&graph.dependencies);
    if cyclic_edges.is_empty() {
        return ops;
    }

    let mut node_by_position = BTreeMap::new();
    for (node_idx, (position, _)) in create_table_positions.iter().enumerate() {
        node_by_position.insert(*position, node_idx);
    }

    let mut create_ops = Vec::with_capacity(ops.len());
    let mut add_fk_ops = Vec::new();

    for (position, op) in ops.into_iter().enumerate() {
        match op {
            DiffOp::CreateTable(mut table) => {
                let node_idx = node_by_position
                    .get(&position)
                    .copied()
                    .expect("create table position must be indexed");
                let source_key = QualifiedNameKey::from(&table.name);
                let mut retained_fks = Vec::with_capacity(table.foreign_keys.len());

                for foreign_key in table.foreign_keys {
                    let target_key = QualifiedNameKey::from(&foreign_key.referenced_table);
                    if target_key == source_key {
                        retained_fks.push(foreign_key);
                        continue;
                    }

                    let Some(target_idx) = graph.index_by_table.get(&target_key).copied() else {
                        retained_fks.push(foreign_key);
                        continue;
                    };

                    if cyclic_edges.contains(&(node_idx, target_idx)) {
                        add_fk_ops.push(DiffOp::AddForeignKey {
                            table: table.name.clone(),
                            fk: foreign_key,
                        });
                    } else {
                        retained_fks.push(foreign_key);
                    }
                }

                table.foreign_keys = retained_fks;
                create_ops.push(DiffOp::CreateTable(table));
            }
            other => create_ops.push(other),
        }
    }

    create_ops.extend(add_fk_ops);
    create_ops
}

pub(super) fn drop_fk_ops_for_drop_table_cycles(tables_to_drop: &[&Table]) -> Vec<DiffOp> {
    if tables_to_drop.len() < 2 {
        return Vec::new();
    }

    let graph = build_fk_dependency_graph(tables_to_drop);
    let cyclic_edges = find_cyclic_edges(&graph.dependencies);
    if cyclic_edges.is_empty() {
        return Vec::new();
    }

    let mut drop_fk_ops = Vec::new();
    for (source_idx, table) in tables_to_drop.iter().enumerate() {
        let source_key = QualifiedNameKey::from(&table.name);
        for foreign_key in &table.foreign_keys {
            let Some(foreign_key_name) = foreign_key.name.clone() else {
                continue;
            };

            let target_key = QualifiedNameKey::from(&foreign_key.referenced_table);
            if target_key == source_key {
                continue;
            }

            let Some(target_idx) = graph.index_by_table.get(&target_key).copied() else {
                continue;
            };

            if cyclic_edges.contains(&(source_idx, target_idx)) {
                drop_fk_ops.push(DiffOp::DropForeignKey {
                    table: table.name.clone(),
                    name: foreign_key_name,
                });
            }
        }
    }

    drop_fk_ops
}

fn build_fk_dependency_graph(tables: &[&Table]) -> FkDependencyGraph {
    let mut index_by_table = BTreeMap::new();
    for (idx, table) in tables.iter().enumerate() {
        index_by_table.insert(QualifiedNameKey::from(&table.name), idx);
    }

    let mut dependencies = vec![BTreeSet::new(); tables.len()];
    for (idx, table) in tables.iter().enumerate() {
        let source_key = QualifiedNameKey::from(&table.name);
        for foreign_key in &table.foreign_keys {
            let target_key = QualifiedNameKey::from(&foreign_key.referenced_table);
            if target_key == source_key {
                continue;
            }

            if let Some(target_idx) = index_by_table.get(&target_key).copied() {
                dependencies[idx].insert(target_idx);
            }
        }
    }

    FkDependencyGraph {
        dependencies,
        index_by_table,
    }
}

fn find_cyclic_edges(dependencies: &[BTreeSet<usize>]) -> BTreeSet<(usize, usize)> {
    let mut cyclic_edges = BTreeSet::new();

    for (source_idx, targets) in dependencies.iter().enumerate() {
        for target_idx in targets {
            if can_reach(*target_idx, source_idx, dependencies) {
                cyclic_edges.insert((source_idx, *target_idx));
            }
        }
    }

    cyclic_edges
}

fn can_reach(start: usize, target: usize, dependencies: &[BTreeSet<usize>]) -> bool {
    let mut stack = vec![start];
    let mut visited = BTreeSet::new();

    while let Some(node_idx) = stack.pop() {
        if node_idx == target {
            return true;
        }

        if !visited.insert(node_idx) {
            continue;
        }

        for next_idx in &dependencies[node_idx] {
            if !visited.contains(next_idx) {
                stack.push(*next_idx);
            }
        }
    }

    false
}
