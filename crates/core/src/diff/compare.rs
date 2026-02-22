use std::collections::{BTreeMap, BTreeSet};

use super::{
    compare_remaining::{compare_remaining_objects, validate_sequence_invariant},
    constraint_pairing::check_drop_add_keys_match,
    enable_drop::{DiffDiagnostics, DiffOutcome},
    name_resolution::{
        IdentKey, IndexLookupKey, IndexOwnerKey, QualifiedNameKey, resolve_index_match,
        resolve_qualified_name_match,
    },
    partition::diff_partition,
    rename::{index_renamed_from, indexes_equivalent_for_rename, resolve_rename_match},
};
use crate::{
    CheckConstraint, Column, ColumnChange, DataType, DiffConfig, DiffError, DiffOp, Ident,
    IndexDef, IndexOwner, QualifiedName, Result, SchemaObject, Table, custom_types_equivalent,
    exprs_equivalent,
};

#[derive(Debug, Default, Clone, Copy)]
pub struct DiffEngine;

impl DiffEngine {
    #[must_use]
    pub const fn new() -> Self {
        Self
    }

    pub fn diff(
        &self,
        desired: &[SchemaObject],
        current: &[SchemaObject],
        config: &DiffConfig,
    ) -> Result<Vec<DiffOp>> {
        let compare_ops = self.compare_objects(desired, current, config)?;
        self.resolve_and_order(compare_ops, config)
    }

    pub fn diff_with_diagnostics(
        &self,
        desired: &[SchemaObject],
        current: &[SchemaObject],
        config: &DiffConfig,
    ) -> Result<DiffOutcome> {
        let ops = self.diff(desired, current, config)?;
        let diagnostics = if config.enable_drop {
            DiffDiagnostics::default()
        } else {
            let mut with_drop_enabled = config.clone();
            with_drop_enabled.enable_drop = true;
            let full_ops = self.diff(desired, current, &with_drop_enabled)?;
            DiffDiagnostics::from_enable_drop(&full_ops, &ops)
        };

        Ok(DiffOutcome::new(ops, diagnostics))
    }

    fn compare_objects(
        &self,
        desired: &[SchemaObject],
        current: &[SchemaObject],
        config: &DiffConfig,
    ) -> Result<Vec<DiffOp>> {
        validate_sequence_invariant(desired, "desired")?;
        validate_sequence_invariant(current, "current")?;

        let desired_objects = ObjectBuckets::from_schema(desired)?;
        let current_objects = ObjectBuckets::from_schema(current)?;

        validate_index_owners(&desired_objects, "desired")?;
        validate_index_owners(&current_objects, "current")?;

        let mut ops = Vec::new();
        self.compare_tables(&desired_objects, &current_objects, config, &mut ops);
        self.compare_indexes(
            &desired_objects.indexes,
            &current_objects.indexes,
            config,
            &mut ops,
        )?;
        compare_remaining_objects(desired, current, config, &mut ops)?;
        Ok(ops)
    }

    fn resolve_and_order(&self, ops: Vec<DiffOp>, _config: &DiffConfig) -> Result<Vec<DiffOp>> {
        Ok(ops)
    }

    fn compare_tables(
        &self,
        desired: &ObjectBuckets<'_>,
        current: &ObjectBuckets<'_>,
        config: &DiffConfig,
        ops: &mut Vec<DiffOp>,
    ) {
        let mut matched_current = BTreeSet::new();

        for (table_key, desired_table) in &desired.tables {
            if let Some(current_table) = current.tables.get(table_key) {
                matched_current.insert((*table_key).clone());
                self.compare_table(desired_table, current_table, config, ops);
                continue;
            }

            if let Some((matched_key, current_table)) = resolve_qualified_name_match(
                table_key,
                &current.tables,
                &matched_current,
                &config.schema_search_path,
            ) {
                matched_current.insert((*matched_key).clone());
                self.compare_table(desired_table, current_table, config, ops);
                continue;
            }

            let renamed_from = table_renamed_from_key(desired_table);
            if let Some((from_key, current_table)) =
                resolve_rename_match(renamed_from.as_ref(), &current.tables, &matched_current)
            {
                matched_current.insert((*from_key).clone());
                ops.push(DiffOp::RenameTable {
                    from: current_table.name.clone(),
                    to: desired_table.name.clone(),
                });
                self.compare_table(desired_table, current_table, config, ops);
            } else {
                ops.push(DiffOp::CreateTable((*desired_table).clone()));
            }
        }

        if config.enable_drop {
            for (table_key, current_table) in &current.tables {
                if !matched_current.contains(table_key) {
                    ops.push(DiffOp::DropTable(current_table.name.clone()));
                }
            }
        }
    }

    fn compare_table(
        &self,
        desired: &Table,
        current: &Table,
        config: &DiffConfig,
        ops: &mut Vec<DiffOp>,
    ) {
        self.compare_columns(
            &desired.name,
            &desired.columns,
            &current.columns,
            config,
            ops,
        );
        self.compare_checks(&desired.name, &desired.checks, &current.checks, config, ops);
        diff_partition(
            &desired.name,
            desired.partition.as_ref(),
            current.partition.as_ref(),
            config,
            ops,
        );
    }

    fn compare_columns(
        &self,
        table: &QualifiedName,
        desired_columns: &[Column],
        current_columns: &[Column],
        config: &DiffConfig,
        ops: &mut Vec<DiffOp>,
    ) {
        let current_by_name = map_columns_by_name(current_columns);
        let desired_by_name = map_columns_by_name(desired_columns);
        let mut matched_current = BTreeSet::new();

        for (column_key, desired_column) in &desired_by_name {
            if let Some(current_column) = current_by_name.get(column_key) {
                matched_current.insert((*column_key).clone());
                let changes = column_changes(desired_column, current_column, config);
                if !changes.is_empty() {
                    ops.push(DiffOp::AlterColumn {
                        table: table.clone(),
                        column: desired_column.name.clone(),
                        changes,
                    });
                }
                continue;
            }

            let renamed_from = column_renamed_from_key(desired_column);
            if let Some((renamed_key, current_column)) =
                resolve_rename_match(renamed_from.as_ref(), &current_by_name, &matched_current)
            {
                matched_current.insert((*renamed_key).clone());
                ops.push(DiffOp::RenameColumn {
                    table: table.clone(),
                    from: current_column.name.clone(),
                    to: desired_column.name.clone(),
                });

                let changes = column_changes(desired_column, current_column, config);
                if !changes.is_empty() {
                    ops.push(DiffOp::AlterColumn {
                        table: table.clone(),
                        column: desired_column.name.clone(),
                        changes,
                    });
                }
            } else {
                ops.push(DiffOp::AddColumn {
                    table: table.clone(),
                    column: Box::new((*desired_column).clone()),
                    position: None,
                });
            }
        }

        if config.enable_drop {
            for (column_key, current_column) in &current_by_name {
                if !desired_by_name.contains_key(column_key)
                    && !matched_current.contains(column_key)
                {
                    ops.push(DiffOp::DropColumn {
                        table: table.clone(),
                        column: current_column.name.clone(),
                    });
                }
            }
        }
    }

    fn compare_checks(
        &self,
        table: &QualifiedName,
        desired_checks: &[CheckConstraint],
        current_checks: &[CheckConstraint],
        config: &DiffConfig,
        ops: &mut Vec<DiffOp>,
    ) {
        let desired_named = map_named_checks(desired_checks);
        let current_named = map_named_checks(current_checks);

        for (check_key, (check_name, desired_check)) in &desired_named {
            match current_named.get(check_key) {
                Some((_, current_check)) => {
                    if !checks_equivalent(desired_check, current_check, config) {
                        if config.enable_drop
                            || check_drop_add_keys_match(table, check_name, desired_check)
                        {
                            ops.push(DiffOp::DropCheck {
                                table: table.clone(),
                                name: check_name.clone(),
                            });
                        }
                        ops.push(DiffOp::AddCheck {
                            table: table.clone(),
                            check: (*desired_check).clone(),
                        });
                    }
                }
                None => {
                    ops.push(DiffOp::AddCheck {
                        table: table.clone(),
                        check: (*desired_check).clone(),
                    });
                }
            }
        }

        if config.enable_drop {
            for (check_key, (check_name, _current_check)) in &current_named {
                if !desired_named.contains_key(check_key) {
                    ops.push(DiffOp::DropCheck {
                        table: table.clone(),
                        name: check_name.clone(),
                    });
                }
            }
        }
    }

    fn compare_indexes(
        &self,
        desired_indexes: &[&IndexDef],
        current_indexes: &[&IndexDef],
        config: &DiffConfig,
        ops: &mut Vec<DiffOp>,
    ) -> Result<()> {
        let desired_by_key = map_indexes_by_key(desired_indexes)?;
        let current_by_key = map_indexes_by_key(current_indexes)?;
        let mut matched_current = BTreeSet::new();

        for (index_key, desired_index) in &desired_by_key {
            if let Some(current_index) = current_by_key.get(index_key) {
                matched_current.insert((*index_key).clone());
                self.push_index_update_ops(desired_index, current_index, config, ops);
                continue;
            }

            if let Some((matched_key, current_index)) = resolve_index_match(
                index_key,
                &current_by_key,
                &matched_current,
                &config.schema_search_path,
            ) {
                matched_current.insert((*matched_key).clone());
                self.push_index_update_ops(desired_index, current_index, config, ops);
                continue;
            }

            let renamed_from_key = index_renamed_from_key(desired_index);
            if let Some((from_key, current_index)) =
                resolve_rename_match(renamed_from_key.as_ref(), &current_by_key, &matched_current)
                && indexes_equivalent_for_rename(desired_index, current_index)
            {
                matched_current.insert((*from_key).clone());
                let to = index_name(desired_index)?;
                let from = index_name(current_index)?;
                ops.push(DiffOp::RenameIndex {
                    owner: desired_index.owner.clone(),
                    from,
                    to,
                });
                continue;
            }

            ops.push(DiffOp::AddIndex((*desired_index).clone()));
        }

        if config.enable_drop {
            for (index_key, current_index) in &current_by_key {
                if !desired_by_key.contains_key(index_key)
                    && !matched_current.contains(index_key)
                    && let Some(name) = &current_index.name
                {
                    ops.push(DiffOp::DropIndex {
                        owner: current_index.owner.clone(),
                        name: name.clone(),
                    });
                }
            }
        }

        Ok(())
    }

    fn push_index_update_ops(
        &self,
        desired_index: &IndexDef,
        current_index: &IndexDef,
        config: &DiffConfig,
        ops: &mut Vec<DiffOp>,
    ) {
        if desired_index == current_index {
            return;
        }

        if config.enable_drop
            && let Some(name) = &current_index.name
        {
            ops.push(DiffOp::DropIndex {
                owner: current_index.owner.clone(),
                name: name.clone(),
            });
        }

        ops.push(DiffOp::AddIndex(desired_index.clone()));
    }
}

fn map_columns_by_name(columns: &[Column]) -> BTreeMap<IdentKey, &Column> {
    let mut columns_by_name = BTreeMap::new();
    for column in columns {
        columns_by_name.insert(IdentKey::from(&column.name), column);
    }
    columns_by_name
}

fn map_named_checks(checks: &[CheckConstraint]) -> BTreeMap<IdentKey, (Ident, &CheckConstraint)> {
    let mut checks_by_name = BTreeMap::new();
    for check in checks {
        if let Some(name) = &check.name {
            checks_by_name.insert(IdentKey::from(name), (name.clone(), check));
        }
    }
    checks_by_name
}

fn map_indexes_by_key<'a>(
    indexes: &[&'a IndexDef],
) -> Result<BTreeMap<IndexLookupKey, &'a IndexDef>> {
    let mut indexes_by_key = BTreeMap::new();
    for index in indexes {
        let key = index_lookup_key(index)?;
        indexes_by_key.insert(key, *index);
    }
    Ok(indexes_by_key)
}

fn table_renamed_from_key(table: &Table) -> Option<QualifiedNameKey> {
    let renamed_from = table.renamed_from.as_ref()?;
    Some(QualifiedNameKey {
        schema: table.name.schema.as_ref().map(IdentKey::from),
        name: IdentKey::from(renamed_from),
    })
}

fn column_renamed_from_key(column: &Column) -> Option<IdentKey> {
    column.renamed_from.as_ref().map(IdentKey::from)
}

fn index_renamed_from_key(index: &IndexDef) -> Option<IndexLookupKey> {
    let renamed_from = index_renamed_from(index)?;
    Some(IndexLookupKey {
        owner: IndexOwnerKey::from(&index.owner),
        name: IdentKey::from(&renamed_from),
    })
}

fn index_name(index: &IndexDef) -> Result<Ident> {
    let Some(name) = &index.name else {
        return Err(DiffError::ObjectComparison {
            target: describe_index_owner(&index.owner),
            operation: "index name is required for diff comparison".to_string(),
        }
        .into());
    };

    Ok(name.clone())
}

fn column_changes(desired: &Column, current: &Column, config: &DiffConfig) -> Vec<ColumnChange> {
    let mut changes = Vec::new();

    if !data_types_equivalent(&desired.data_type, &current.data_type, config) {
        changes.push(ColumnChange::SetType(desired.data_type.clone()));
    }

    if desired.not_null != current.not_null {
        changes.push(ColumnChange::SetNotNull(desired.not_null));
    }

    if !optional_exprs_equivalent(desired.default.as_ref(), current.default.as_ref(), config) {
        changes.push(ColumnChange::SetDefault(desired.default.clone()));
    }

    changes
}

fn data_types_equivalent(desired: &DataType, current: &DataType, config: &DiffConfig) -> bool {
    match (desired, current) {
        (DataType::Custom(left), DataType::Custom(right)) => {
            custom_types_equivalent(config.equivalence_policy.as_ref(), left, right)
        }
        _ => desired == current,
    }
}

fn optional_exprs_equivalent(
    desired: Option<&crate::Expr>,
    current: Option<&crate::Expr>,
    config: &DiffConfig,
) -> bool {
    match (desired, current) {
        (Some(desired_expr), Some(current_expr)) => exprs_equivalent(
            config.equivalence_policy.as_ref(),
            desired_expr,
            current_expr,
        ),
        (None, None) => true,
        _ => false,
    }
}

fn checks_equivalent(
    desired: &CheckConstraint,
    current: &CheckConstraint,
    config: &DiffConfig,
) -> bool {
    desired.no_inherit == current.no_inherit
        && exprs_equivalent(
            config.equivalence_policy.as_ref(),
            &desired.expr,
            &current.expr,
        )
}

fn validate_index_owners(objects: &ObjectBuckets<'_>, side: &str) -> Result<()> {
    for index in &objects.indexes {
        let owner_exists = match &index.owner {
            IndexOwner::Table(owner) => objects.tables.contains_key(&QualifiedNameKey::from(owner)),
            IndexOwner::View(owner) => objects.views.contains(&QualifiedNameKey::from(owner)),
            IndexOwner::MaterializedView(owner) => objects
                .materialized_views
                .contains(&QualifiedNameKey::from(owner)),
        };

        if !owner_exists {
            return Err(DiffError::ObjectComparison {
                target: describe_index_owner(&index.owner),
                operation: format!("index owner not found in {side} schema"),
            }
            .into());
        }
    }

    Ok(())
}

fn index_lookup_key(index: &IndexDef) -> Result<IndexLookupKey> {
    let Some(name) = &index.name else {
        return Err(DiffError::ObjectComparison {
            target: describe_index_owner(&index.owner),
            operation: "index name is required for diff comparison".to_string(),
        }
        .into());
    };

    Ok(IndexLookupKey {
        owner: IndexOwnerKey::from(&index.owner),
        name: IdentKey::from(name),
    })
}

fn describe_index_owner(owner: &IndexOwner) -> String {
    match owner {
        IndexOwner::Table(name) => format!("table {}", display_qualified_name(name)),
        IndexOwner::View(name) => format!("view {}", display_qualified_name(name)),
        IndexOwner::MaterializedView(name) => {
            format!("materialized view {}", display_qualified_name(name))
        }
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

#[derive(Debug)]
struct ObjectBuckets<'a> {
    tables: BTreeMap<QualifiedNameKey, &'a Table>,
    views: BTreeSet<QualifiedNameKey>,
    materialized_views: BTreeSet<QualifiedNameKey>,
    indexes: Vec<&'a IndexDef>,
}

impl<'a> ObjectBuckets<'a> {
    fn from_schema(objects: &'a [SchemaObject]) -> Result<Self> {
        let mut tables = BTreeMap::new();
        let mut views = BTreeSet::new();
        let mut materialized_views = BTreeSet::new();
        let mut indexes = Vec::new();

        for object in objects {
            match object {
                SchemaObject::Table(table) => {
                    tables.insert(QualifiedNameKey::from(&table.name), table);
                }
                SchemaObject::View(view) => {
                    views.insert(QualifiedNameKey::from(&view.name));
                }
                SchemaObject::MaterializedView(view) => {
                    materialized_views.insert(QualifiedNameKey::from(&view.name));
                }
                SchemaObject::Index(index) => {
                    indexes.push(index);
                }
                SchemaObject::Sequence(_)
                | SchemaObject::Trigger(_)
                | SchemaObject::Function(_)
                | SchemaObject::Type(_)
                | SchemaObject::Domain(_)
                | SchemaObject::Extension(_)
                | SchemaObject::Schema(_)
                | SchemaObject::Comment(_)
                | SchemaObject::Privilege(_)
                | SchemaObject::Policy(_) => {}
            }
        }

        Ok(Self {
            tables,
            views,
            materialized_views,
            indexes,
        })
    }
}
