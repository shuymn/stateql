use std::collections::BTreeMap;

use super::{
    name_resolution::QualifiedNameKey, privilege::compare_privileges,
    view_rebuild::build_view_rebuild_plan,
};
use crate::{
    CheckConstraint, Comment, DiffConfig, DiffError, DiffOp, Domain, DomainChange, Extension,
    Function, Ident, MaterializedView, Policy, Privilege, QualifiedName, Result, SchemaDef,
    SchemaObject, Sequence, SequenceChange, Table, Trigger, TypeChange, TypeDef, TypeKind, View,
};

pub(crate) fn compare_remaining_objects(
    desired: &[SchemaObject],
    current: &[SchemaObject],
    config: &DiffConfig,
    ops: &mut Vec<DiffOp>,
) -> Result<()> {
    let desired_views = collect_views(desired);
    let current_views = collect_views(current);
    compare_views(&desired_views, &current_views, config, ops);

    let desired_materialized_views = collect_materialized_views(desired);
    let current_materialized_views = collect_materialized_views(current);
    compare_materialized_views(
        &desired_materialized_views,
        &current_materialized_views,
        config,
        ops,
    );

    let desired_sequences = collect_sequences(desired);
    let current_sequences = collect_sequences(current);
    compare_sequences(&desired_sequences, &current_sequences, config, ops);

    let desired_triggers = collect_triggers(desired);
    let current_triggers = collect_triggers(current);
    compare_triggers(&desired_triggers, &current_triggers, config, ops);

    let desired_functions = collect_functions(desired);
    let current_functions = collect_functions(current);
    compare_functions(&desired_functions, &current_functions, config, ops);

    let desired_types = collect_types(desired);
    let current_types = collect_types(current);
    compare_types(&desired_types, &current_types, config, ops);

    let desired_domains = collect_domains(desired);
    let current_domains = collect_domains(current);
    compare_domains(&desired_domains, &current_domains, config, ops);

    let desired_extensions = collect_extensions(desired);
    let current_extensions = collect_extensions(current);
    compare_extensions(&desired_extensions, &current_extensions, config, ops);

    let desired_schemas = collect_schemas(desired);
    let current_schemas = collect_schemas(current);
    compare_schemas(&desired_schemas, &current_schemas, config, ops);

    let desired_comments = collect_comments(desired);
    let current_comments = collect_comments(current);
    compare_comments(&desired_comments, &current_comments, config, ops);

    let desired_privileges = collect_privileges(desired);
    let current_privileges = collect_privileges(current);
    compare_privileges(&desired_privileges, &current_privileges, config, ops);

    let desired_policies = collect_policies(desired);
    let current_policies = collect_policies(current);
    compare_policies(&desired_policies, &current_policies, config, ops);

    Ok(())
}

pub(crate) fn validate_sequence_invariant(objects: &[SchemaObject], side: &str) -> Result<()> {
    let explicit_sequences = collect_sequences(objects)
        .into_iter()
        .map(|sequence| sequence.name.clone())
        .collect::<Vec<_>>();

    for table in collect_tables(objects) {
        for column in &table.columns {
            if column.identity.is_none() {
                continue;
            }

            let implicit_sequence = implicit_identity_sequence_name(&table.name, &column.name);
            if explicit_sequences.contains(&implicit_sequence) {
                return Err(DiffError::ObjectComparison {
                    target: display_qualified_name(&implicit_sequence),
                    operation: format!(
                        "sequence duplicate invariant violation in {side} schema: explicit sequence overlaps implicit identity sequence for {}.{}",
                        display_qualified_name(&table.name),
                        display_ident(&column.name),
                    ),
                }
                .into());
            }
        }
    }

    Ok(())
}

fn collect_tables(objects: &[SchemaObject]) -> Vec<&Table> {
    objects
        .iter()
        .filter_map(|object| match object {
            SchemaObject::Table(table) => Some(table),
            _ => None,
        })
        .collect()
}

fn collect_views(objects: &[SchemaObject]) -> Vec<&View> {
    objects
        .iter()
        .filter_map(|object| match object {
            SchemaObject::View(view) => Some(view),
            _ => None,
        })
        .collect()
}

fn map_views_by_name<'a>(views: &[&'a View]) -> BTreeMap<QualifiedNameKey, &'a View> {
    let mut views_by_name = BTreeMap::new();
    for view in views {
        views_by_name.insert(QualifiedNameKey::from(&view.name), *view);
    }
    views_by_name
}

fn collect_materialized_views(objects: &[SchemaObject]) -> Vec<&MaterializedView> {
    objects
        .iter()
        .filter_map(|object| match object {
            SchemaObject::MaterializedView(view) => Some(view),
            _ => None,
        })
        .collect()
}

fn collect_sequences(objects: &[SchemaObject]) -> Vec<&Sequence> {
    objects
        .iter()
        .filter_map(|object| match object {
            SchemaObject::Sequence(sequence) => Some(sequence),
            _ => None,
        })
        .collect()
}

fn collect_triggers(objects: &[SchemaObject]) -> Vec<&Trigger> {
    objects
        .iter()
        .filter_map(|object| match object {
            SchemaObject::Trigger(trigger) => Some(trigger),
            _ => None,
        })
        .collect()
}

fn collect_functions(objects: &[SchemaObject]) -> Vec<&Function> {
    objects
        .iter()
        .filter_map(|object| match object {
            SchemaObject::Function(function) => Some(function),
            _ => None,
        })
        .collect()
}

fn collect_types(objects: &[SchemaObject]) -> Vec<&TypeDef> {
    objects
        .iter()
        .filter_map(|object| match object {
            SchemaObject::Type(type_def) => Some(type_def),
            _ => None,
        })
        .collect()
}

fn collect_domains(objects: &[SchemaObject]) -> Vec<&Domain> {
    objects
        .iter()
        .filter_map(|object| match object {
            SchemaObject::Domain(domain) => Some(domain),
            _ => None,
        })
        .collect()
}

fn collect_extensions(objects: &[SchemaObject]) -> Vec<&Extension> {
    objects
        .iter()
        .filter_map(|object| match object {
            SchemaObject::Extension(extension) => Some(extension),
            _ => None,
        })
        .collect()
}

fn collect_schemas(objects: &[SchemaObject]) -> Vec<&SchemaDef> {
    objects
        .iter()
        .filter_map(|object| match object {
            SchemaObject::Schema(schema) => Some(schema),
            _ => None,
        })
        .collect()
}

fn collect_comments(objects: &[SchemaObject]) -> Vec<&Comment> {
    objects
        .iter()
        .filter_map(|object| match object {
            SchemaObject::Comment(comment) => Some(comment),
            _ => None,
        })
        .collect()
}

fn collect_policies(objects: &[SchemaObject]) -> Vec<&Policy> {
    objects
        .iter()
        .filter_map(|object| match object {
            SchemaObject::Policy(policy) => Some(policy),
            _ => None,
        })
        .collect()
}

fn collect_privileges(objects: &[SchemaObject]) -> Vec<&Privilege> {
    objects
        .iter()
        .filter_map(|object| match object {
            SchemaObject::Privilege(privilege) => Some(privilege),
            _ => None,
        })
        .collect()
}

fn compare_views(desired: &[&View], current: &[&View], config: &DiffConfig, ops: &mut Vec<DiffOp>) {
    let desired_by_key = map_views_by_name(desired);
    let current_by_key = map_views_by_name(current);
    let rebuild_plan = build_view_rebuild_plan(&desired_by_key, &current_by_key);

    if config.enable_drop {
        for drop_key in &rebuild_plan.drop_order {
            if let Some(current_view) = current_by_key.get(drop_key) {
                ops.push(DiffOp::DropView(current_view.name.clone()));
            }
        }
    }

    for create_key in &rebuild_plan.create_order {
        if let Some(desired_view) = desired_by_key.get(create_key) {
            ops.push(DiffOp::CreateView((*desired_view).clone()));
        }
    }

    for desired_view in desired.iter().copied() {
        let view_key = QualifiedNameKey::from(&desired_view.name);
        if rebuild_plan.rebuild_set.contains(&view_key) {
            continue;
        }

        if !current_by_key.contains_key(&view_key) {
            ops.push(DiffOp::CreateView(desired_view.clone()));
        }
    }

    if config.enable_drop {
        for current_view in current.iter().copied() {
            let view_key = QualifiedNameKey::from(&current_view.name);
            if rebuild_plan.rebuild_set.contains(&view_key) {
                continue;
            }

            if !desired_by_key.contains_key(&view_key) {
                ops.push(DiffOp::DropView(current_view.name.clone()));
            }
        }
    }
}

fn compare_materialized_views(
    desired: &[&MaterializedView],
    current: &[&MaterializedView],
    config: &DiffConfig,
    ops: &mut Vec<DiffOp>,
) {
    for desired_view in desired.iter().copied() {
        match current
            .iter()
            .copied()
            .find(|candidate| candidate.name == desired_view.name)
        {
            Some(current_view) => {
                if desired_view != current_view {
                    if config.enable_drop {
                        ops.push(DiffOp::DropMaterializedView(current_view.name.clone()));
                    }
                    ops.push(DiffOp::CreateMaterializedView(desired_view.clone()));
                }
            }
            None => ops.push(DiffOp::CreateMaterializedView(desired_view.clone())),
        }
    }

    if config.enable_drop {
        for current_view in current.iter().copied() {
            let missing_in_desired = desired
                .iter()
                .copied()
                .all(|candidate| candidate.name != current_view.name);
            if missing_in_desired {
                ops.push(DiffOp::DropMaterializedView(current_view.name.clone()));
            }
        }
    }
}

fn compare_sequences(
    desired: &[&Sequence],
    current: &[&Sequence],
    config: &DiffConfig,
    ops: &mut Vec<DiffOp>,
) {
    for desired_sequence in desired.iter().copied() {
        match current
            .iter()
            .copied()
            .find(|candidate| candidate.name == desired_sequence.name)
        {
            Some(current_sequence) => {
                if desired_sequence == current_sequence {
                    continue;
                }

                match sequence_changes(desired_sequence, current_sequence) {
                    Some(changes) if !changes.is_empty() => ops.push(DiffOp::AlterSequence {
                        name: desired_sequence.name.clone(),
                        changes,
                    }),
                    Some(_) => {}
                    None => {
                        if config.enable_drop {
                            ops.push(DiffOp::DropSequence(current_sequence.name.clone()));
                        }
                        ops.push(DiffOp::CreateSequence(desired_sequence.clone()));
                    }
                }
            }
            None => ops.push(DiffOp::CreateSequence(desired_sequence.clone())),
        }
    }

    if config.enable_drop {
        for current_sequence in current.iter().copied() {
            let missing_in_desired = desired
                .iter()
                .copied()
                .all(|candidate| candidate.name != current_sequence.name);
            if missing_in_desired {
                ops.push(DiffOp::DropSequence(current_sequence.name.clone()));
            }
        }
    }
}

fn compare_triggers(
    desired: &[&Trigger],
    current: &[&Trigger],
    config: &DiffConfig,
    ops: &mut Vec<DiffOp>,
) {
    for desired_trigger in desired.iter().copied() {
        match current
            .iter()
            .copied()
            .find(|candidate| candidate.name == desired_trigger.name)
        {
            Some(current_trigger) => {
                if desired_trigger != current_trigger {
                    if config.enable_drop {
                        ops.push(DiffOp::DropTrigger {
                            name: current_trigger.name.clone(),
                            table: Some(current_trigger.table.clone()),
                        });
                    }
                    ops.push(DiffOp::CreateTrigger(desired_trigger.clone()));
                }
            }
            None => ops.push(DiffOp::CreateTrigger(desired_trigger.clone())),
        }
    }

    if config.enable_drop {
        for current_trigger in current.iter().copied() {
            let missing_in_desired = desired
                .iter()
                .copied()
                .all(|candidate| candidate.name != current_trigger.name);
            if missing_in_desired {
                ops.push(DiffOp::DropTrigger {
                    name: current_trigger.name.clone(),
                    table: Some(current_trigger.table.clone()),
                });
            }
        }
    }
}

fn compare_functions(
    desired: &[&Function],
    current: &[&Function],
    config: &DiffConfig,
    ops: &mut Vec<DiffOp>,
) {
    for desired_function in desired.iter().copied() {
        match current
            .iter()
            .copied()
            .find(|candidate| candidate.name == desired_function.name)
        {
            Some(current_function) => {
                if desired_function != current_function {
                    if config.enable_drop {
                        ops.push(DiffOp::DropFunction(current_function.name.clone()));
                    }
                    ops.push(DiffOp::CreateFunction(desired_function.clone()));
                }
            }
            None => ops.push(DiffOp::CreateFunction(desired_function.clone())),
        }
    }

    if config.enable_drop {
        for current_function in current.iter().copied() {
            let missing_in_desired = desired
                .iter()
                .copied()
                .all(|candidate| candidate.name != current_function.name);
            if missing_in_desired {
                ops.push(DiffOp::DropFunction(current_function.name.clone()));
            }
        }
    }
}

fn compare_types(
    desired: &[&TypeDef],
    current: &[&TypeDef],
    config: &DiffConfig,
    ops: &mut Vec<DiffOp>,
) {
    for desired_type in desired.iter().copied() {
        match current
            .iter()
            .copied()
            .find(|candidate| candidate.name == desired_type.name)
        {
            Some(current_type) => {
                if desired_type == current_type {
                    continue;
                }

                match type_changes(desired_type, current_type) {
                    Some(changes) if !changes.is_empty() => {
                        for change in changes {
                            ops.push(DiffOp::AlterType {
                                name: desired_type.name.clone(),
                                change,
                            });
                        }
                    }
                    Some(_) => {}
                    None => {
                        if config.enable_drop {
                            ops.push(DiffOp::DropType(current_type.name.clone()));
                        }
                        ops.push(DiffOp::CreateType(desired_type.clone()));
                    }
                }
            }
            None => ops.push(DiffOp::CreateType(desired_type.clone())),
        }
    }

    if config.enable_drop {
        for current_type in current.iter().copied() {
            let missing_in_desired = desired
                .iter()
                .copied()
                .all(|candidate| candidate.name != current_type.name);
            if missing_in_desired {
                ops.push(DiffOp::DropType(current_type.name.clone()));
            }
        }
    }
}

fn compare_domains(
    desired: &[&Domain],
    current: &[&Domain],
    config: &DiffConfig,
    ops: &mut Vec<DiffOp>,
) {
    for desired_domain in desired.iter().copied() {
        match current
            .iter()
            .copied()
            .find(|candidate| candidate.name == desired_domain.name)
        {
            Some(current_domain) => {
                if desired_domain == current_domain {
                    continue;
                }

                match domain_changes(desired_domain, current_domain, config) {
                    Some(changes) if !changes.is_empty() => {
                        for change in changes {
                            ops.push(DiffOp::AlterDomain {
                                name: desired_domain.name.clone(),
                                change,
                            });
                        }
                    }
                    Some(_) => {}
                    None => {
                        if config.enable_drop {
                            ops.push(DiffOp::DropDomain(current_domain.name.clone()));
                        }
                        ops.push(DiffOp::CreateDomain(desired_domain.clone()));
                    }
                }
            }
            None => ops.push(DiffOp::CreateDomain(desired_domain.clone())),
        }
    }

    if config.enable_drop {
        for current_domain in current.iter().copied() {
            let missing_in_desired = desired
                .iter()
                .copied()
                .all(|candidate| candidate.name != current_domain.name);
            if missing_in_desired {
                ops.push(DiffOp::DropDomain(current_domain.name.clone()));
            }
        }
    }
}

fn compare_extensions(
    desired: &[&Extension],
    current: &[&Extension],
    config: &DiffConfig,
    ops: &mut Vec<DiffOp>,
) {
    for desired_extension in desired.iter().copied() {
        match current
            .iter()
            .copied()
            .find(|candidate| candidate.name == desired_extension.name)
        {
            Some(current_extension) => {
                if desired_extension != current_extension {
                    if config.enable_drop {
                        ops.push(DiffOp::DropExtension(extension_name(current_extension)));
                    }
                    ops.push(DiffOp::CreateExtension(desired_extension.clone()));
                }
            }
            None => ops.push(DiffOp::CreateExtension(desired_extension.clone())),
        }
    }

    if config.enable_drop {
        for current_extension in current.iter().copied() {
            let missing_in_desired = desired
                .iter()
                .copied()
                .all(|candidate| candidate.name != current_extension.name);
            if missing_in_desired {
                ops.push(DiffOp::DropExtension(extension_name(current_extension)));
            }
        }
    }
}

fn compare_schemas(
    desired: &[&SchemaDef],
    current: &[&SchemaDef],
    config: &DiffConfig,
    ops: &mut Vec<DiffOp>,
) {
    for desired_schema in desired.iter().copied() {
        let present_in_current = current
            .iter()
            .copied()
            .any(|candidate| candidate.name == desired_schema.name);
        if !present_in_current {
            ops.push(DiffOp::CreateSchema(desired_schema.clone()));
        }
    }

    if config.enable_drop {
        for current_schema in current.iter().copied() {
            let present_in_desired = desired
                .iter()
                .copied()
                .any(|candidate| candidate.name == current_schema.name);
            if !present_in_desired {
                ops.push(DiffOp::DropSchema(schema_name(current_schema)));
            }
        }
    }
}

fn compare_comments(
    desired: &[&Comment],
    current: &[&Comment],
    config: &DiffConfig,
    ops: &mut Vec<DiffOp>,
) {
    for desired_comment in desired.iter().copied() {
        match current
            .iter()
            .copied()
            .find(|candidate| candidate.target == desired_comment.target)
        {
            Some(current_comment) => {
                if desired_comment.text != current_comment.text {
                    if desired_comment.text.is_some() {
                        ops.push(DiffOp::SetComment(desired_comment.clone()));
                    } else if config.enable_drop {
                        ops.push(DiffOp::DropComment {
                            target: desired_comment.target.clone(),
                        });
                    }
                }
            }
            None => {
                if desired_comment.text.is_some() {
                    ops.push(DiffOp::SetComment(desired_comment.clone()));
                }
            }
        }
    }

    if config.enable_drop {
        for current_comment in current.iter().copied() {
            let missing_in_desired = desired
                .iter()
                .copied()
                .all(|candidate| candidate.target != current_comment.target);
            if missing_in_desired && current_comment.text.is_some() {
                ops.push(DiffOp::DropComment {
                    target: current_comment.target.clone(),
                });
            }
        }
    }
}

fn compare_policies(
    desired: &[&Policy],
    current: &[&Policy],
    config: &DiffConfig,
    ops: &mut Vec<DiffOp>,
) {
    for desired_policy in desired.iter().copied() {
        match current.iter().copied().find(|candidate| {
            candidate.name == desired_policy.name && candidate.table == desired_policy.table
        }) {
            Some(current_policy) => {
                if desired_policy != current_policy {
                    if config.enable_drop {
                        ops.push(DiffOp::DropPolicy {
                            name: current_policy.name.clone(),
                            table: current_policy.table.clone(),
                        });
                    }
                    ops.push(DiffOp::CreatePolicy(desired_policy.clone()));
                }
            }
            None => ops.push(DiffOp::CreatePolicy(desired_policy.clone())),
        }
    }

    if config.enable_drop {
        for current_policy in current.iter().copied() {
            let missing_in_desired = desired.iter().copied().all(|candidate| {
                candidate.name != current_policy.name || candidate.table != current_policy.table
            });
            if missing_in_desired {
                ops.push(DiffOp::DropPolicy {
                    name: current_policy.name.clone(),
                    table: current_policy.table.clone(),
                });
            }
        }
    }
}

fn sequence_changes(desired: &Sequence, current: &Sequence) -> Option<Vec<SequenceChange>> {
    let mut changes = Vec::new();

    if desired.data_type != current.data_type {
        match desired.data_type.clone() {
            Some(data_type) => changes.push(SequenceChange::SetType(data_type)),
            None => return None,
        }
    }

    if desired.increment != current.increment {
        match desired.increment {
            Some(increment) => changes.push(SequenceChange::SetIncrement(increment)),
            None => return None,
        }
    }

    if desired.min_value != current.min_value {
        changes.push(SequenceChange::SetMinValue(desired.min_value));
    }

    if desired.max_value != current.max_value {
        changes.push(SequenceChange::SetMaxValue(desired.max_value));
    }

    if desired.start != current.start {
        match desired.start {
            Some(start) => changes.push(SequenceChange::SetStart(start)),
            None => return None,
        }
    }

    if desired.cache != current.cache {
        match desired.cache {
            Some(cache) => changes.push(SequenceChange::SetCache(cache)),
            None => return None,
        }
    }

    if desired.cycle != current.cycle {
        changes.push(SequenceChange::SetCycle(desired.cycle));
    }

    if desired.owned_by != current.owned_by {
        return None;
    }

    Some(changes)
}

fn type_changes(desired: &TypeDef, current: &TypeDef) -> Option<Vec<TypeChange>> {
    if desired.kind == current.kind {
        return Some(Vec::new());
    }

    match (&desired.kind, &current.kind) {
        (
            TypeKind::Enum {
                labels: desired_labels,
            },
            TypeKind::Enum {
                labels: current_labels,
            },
        ) => {
            if desired_labels.starts_with(current_labels) {
                let mut changes = Vec::new();
                for label in desired_labels.iter().skip(current_labels.len()) {
                    changes.push(TypeChange::AddValue {
                        value: label.clone(),
                        position: None,
                    });
                }
                return Some(changes);
            }

            if desired_labels.len() == current_labels.len() {
                let mut differences = current_labels
                    .iter()
                    .zip(desired_labels.iter())
                    .filter(|(current_label, desired_label)| current_label != desired_label);

                if let Some((from, to)) = differences.next()
                    && differences.next().is_none()
                {
                    return Some(vec![TypeChange::RenameValue {
                        from: from.clone(),
                        to: to.clone(),
                    }]);
                }
            }

            None
        }
        _ => None,
    }
}

fn domain_changes(
    desired: &Domain,
    current: &Domain,
    config: &DiffConfig,
) -> Option<Vec<DomainChange>> {
    if desired.data_type != current.data_type {
        return None;
    }

    let mut changes = Vec::new();

    if desired.default != current.default {
        changes.push(DomainChange::SetDefault(desired.default.clone()));
    }

    if desired.not_null != current.not_null {
        changes.push(DomainChange::SetNotNull(desired.not_null));
    }

    let has_unnamed_constraint = desired.checks.iter().any(|check| check.name.is_none())
        || current.checks.iter().any(|check| check.name.is_none());

    if has_unnamed_constraint && desired.checks != current.checks {
        return None;
    }

    append_domain_constraint_changes(desired, current, config, &mut changes);
    Some(changes)
}

fn append_domain_constraint_changes(
    desired: &Domain,
    current: &Domain,
    config: &DiffConfig,
    changes: &mut Vec<DomainChange>,
) {
    let desired_named_checks = desired
        .checks
        .iter()
        .filter(|check| check.name.is_some())
        .collect::<Vec<_>>();
    let current_named_checks = current
        .checks
        .iter()
        .filter(|check| check.name.is_some())
        .collect::<Vec<_>>();

    for desired_check in desired_named_checks.iter().copied() {
        let Some(desired_name) = desired_check.name.as_ref() else {
            continue;
        };

        match find_named_check(&current_named_checks, desired_name) {
            Some(current_check) => {
                if desired_check != current_check {
                    if config.enable_drop {
                        changes.push(DomainChange::DropConstraint(desired_name.clone()));
                    }
                    changes.push(DomainChange::AddConstraint {
                        name: Some(desired_name.clone()),
                        check: desired_check.expr.clone(),
                    });
                }
            }
            None => changes.push(DomainChange::AddConstraint {
                name: Some(desired_name.clone()),
                check: desired_check.expr.clone(),
            }),
        }
    }

    if config.enable_drop {
        for current_check in current_named_checks.iter().copied() {
            let Some(current_name) = current_check.name.as_ref() else {
                continue;
            };

            let missing_in_desired =
                find_named_check(&desired_named_checks, current_name).is_none();
            if missing_in_desired {
                changes.push(DomainChange::DropConstraint(current_name.clone()));
            }
        }
    }
}

fn find_named_check<'a>(
    checks: &[&'a CheckConstraint],
    name: &Ident,
) -> Option<&'a CheckConstraint> {
    checks
        .iter()
        .copied()
        .find(|check| check.name.as_ref() == Some(name))
}

fn implicit_identity_sequence_name(table: &QualifiedName, column: &Ident) -> QualifiedName {
    QualifiedName {
        schema: table.schema.clone(),
        name: Ident::unquoted(format!("{}_{}_seq", table.name.value, column.value)),
    }
}

fn display_qualified_name(name: &QualifiedName) -> String {
    match &name.schema {
        Some(schema) => format!("{}.{}", display_ident(schema), display_ident(&name.name)),
        None => display_ident(&name.name),
    }
}

fn display_ident(ident: &Ident) -> String {
    if ident.quoted {
        format!("\"{}\"", ident.value)
    } else {
        ident.value.clone()
    }
}

fn extension_name(extension: &Extension) -> QualifiedName {
    QualifiedName {
        schema: extension.schema.clone(),
        name: extension.name.clone(),
    }
}

fn schema_name(schema: &SchemaDef) -> QualifiedName {
    QualifiedName {
        schema: None,
        name: schema.name.clone(),
    }
}
