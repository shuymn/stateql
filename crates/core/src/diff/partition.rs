use std::collections::BTreeMap;

use crate::{
    DiffConfig, DiffOp, Ident, Partition, PartitionElement, PartitionStrategy, QualifiedName,
};

pub(super) fn diff_partition(
    table: &QualifiedName,
    desired: Option<&Partition>,
    current: Option<&Partition>,
    config: &DiffConfig,
    ops: &mut Vec<DiffOp>,
) {
    match (desired, current) {
        (Some(desired_partition), None) => {
            ops.push(DiffOp::AddPartition {
                table: table.clone(),
                partition: desired_partition.clone(),
            });
        }
        (None, Some(current_partition)) => {
            if config.enable_drop {
                emit_partition_drops(table, &current_partition.partitions, ops);
            }
        }
        (Some(desired_partition), Some(current_partition)) => {
            compare_partition_updates(table, desired_partition, current_partition, config, ops)
        }
        (None, None) => {}
    }
}

fn compare_partition_updates(
    table: &QualifiedName,
    desired: &Partition,
    current: &Partition,
    config: &DiffConfig,
    ops: &mut Vec<DiffOp>,
) {
    if partition_key(desired) != partition_key(current) {
        if config.enable_drop {
            emit_partition_drops(table, &current.partitions, ops);
        }
        ops.push(DiffOp::AddPartition {
            table: table.clone(),
            partition: desired.clone(),
        });
        return;
    }

    let desired_by_name = map_elements_by_name(&desired.partitions);
    let current_by_name = map_elements_by_name(&current.partitions);

    for (name, desired_element) in &desired_by_name {
        match current_by_name.get(name) {
            Some(current_element) => {
                if partition_element_changed(desired_element, current_element) {
                    if config.enable_drop {
                        ops.push(DiffOp::DropPartition {
                            table: table.clone(),
                            name: current_element.name.clone(),
                        });
                    }
                    ops.push(add_partition_for_element(
                        table,
                        desired,
                        (*desired_element).clone(),
                    ));
                }
            }
            None => ops.push(add_partition_for_element(
                table,
                desired,
                (*desired_element).clone(),
            )),
        }
    }

    if config.enable_drop {
        for (name, current_element) in &current_by_name {
            if !desired_by_name.contains_key(name) {
                ops.push(DiffOp::DropPartition {
                    table: table.clone(),
                    name: current_element.name.clone(),
                });
            }
        }
    }
}

fn partition_key(partition: &Partition) -> PartitionKey {
    PartitionKey {
        strategy: partition.strategy.clone(),
        columns: normalize_partition_columns(&partition.columns),
    }
}

fn normalize_partition_columns(columns: &[Ident]) -> Vec<IdentKey> {
    columns.iter().map(IdentKey::from).collect()
}

fn map_elements_by_name(elements: &[PartitionElement]) -> BTreeMap<IdentKey, &PartitionElement> {
    let mut elements_by_name = BTreeMap::new();
    for element in elements {
        elements_by_name.insert(IdentKey::from(&element.name), element);
    }
    elements_by_name
}

fn partition_element_changed(desired: &PartitionElement, current: &PartitionElement) -> bool {
    desired.bound != current.bound || desired.extra != current.extra
}

fn add_partition_for_element(
    table: &QualifiedName,
    source_partition: &Partition,
    element: PartitionElement,
) -> DiffOp {
    DiffOp::AddPartition {
        table: table.clone(),
        partition: Partition {
            strategy: source_partition.strategy.clone(),
            columns: source_partition.columns.clone(),
            partitions: vec![element],
        },
    }
}

fn emit_partition_drops(
    table: &QualifiedName,
    elements: &[PartitionElement],
    ops: &mut Vec<DiffOp>,
) {
    for element in elements {
        ops.push(DiffOp::DropPartition {
            table: table.clone(),
            name: element.name.clone(),
        });
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct PartitionKey {
    strategy: PartitionStrategy,
    columns: Vec<IdentKey>,
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
