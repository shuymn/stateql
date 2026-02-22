use std::collections::{BTreeMap, BTreeSet};

use super::{
    compare_remaining::{compare_remaining_objects, validate_sequence_invariant},
    partition::diff_partition,
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
        for (table_key, desired_table) in &desired.tables {
            match current.tables.get(table_key) {
                Some(current_table) => {
                    self.compare_table(desired_table, current_table, config, ops)
                }
                None => ops.push(DiffOp::CreateTable((*desired_table).clone())),
            }
        }

        if config.enable_drop {
            for (table_key, current_table) in &current.tables {
                if !desired.tables.contains_key(table_key) {
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

        for (column_key, desired_column) in &desired_by_name {
            match current_by_name.get(column_key) {
                Some(current_column) => {
                    let changes = column_changes(desired_column, current_column, config);
                    if !changes.is_empty() {
                        ops.push(DiffOp::AlterColumn {
                            table: table.clone(),
                            column: desired_column.name.clone(),
                            changes,
                        });
                    }
                }
                None => {
                    ops.push(DiffOp::AddColumn {
                        table: table.clone(),
                        column: Box::new((*desired_column).clone()),
                        position: None,
                    });
                }
            }
        }

        if config.enable_drop {
            for (column_key, current_column) in &current_by_name {
                if !desired_by_name.contains_key(column_key) {
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
                        if config.enable_drop {
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

        for (index_key, desired_index) in &desired_by_key {
            match current_by_key.get(index_key) {
                Some(current_index) => {
                    if desired_index != current_index {
                        if config.enable_drop
                            && let Some(name) = &current_index.name
                        {
                            ops.push(DiffOp::DropIndex {
                                owner: current_index.owner.clone(),
                                name: name.clone(),
                            });
                        }
                        ops.push(DiffOp::AddIndex((*desired_index).clone()));
                    }
                }
                None => ops.push(DiffOp::AddIndex((*desired_index).clone())),
            }
        }

        if config.enable_drop {
            for (index_key, current_index) in &current_by_key {
                if !desired_by_key.contains_key(index_key)
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
                | SchemaObject::Policy(_) => {}
                SchemaObject::Privilege(_) => {
                    return Err(DiffError::ObjectComparison {
                        target: schema_object_kind(object).to_string(),
                        operation: "diff comparison does not support this object kind yet"
                            .to_string(),
                    }
                    .into());
                }
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

fn schema_object_kind(object: &SchemaObject) -> &'static str {
    match object {
        SchemaObject::Table(_) => "table",
        SchemaObject::View(_) => "view",
        SchemaObject::MaterializedView(_) => "materialized_view",
        SchemaObject::Index(_) => "index",
        SchemaObject::Sequence(_) => "sequence",
        SchemaObject::Trigger(_) => "trigger",
        SchemaObject::Function(_) => "function",
        SchemaObject::Type(_) => "type",
        SchemaObject::Domain(_) => "domain",
        SchemaObject::Extension(_) => "extension",
        SchemaObject::Schema(_) => "schema",
        SchemaObject::Comment(_) => "comment",
        SchemaObject::Privilege(_) => "privilege",
        SchemaObject::Policy(_) => "policy",
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct IndexLookupKey {
    owner: IndexOwnerKey,
    name: IdentKey,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
enum IndexOwnerKey {
    Table(QualifiedNameKey),
    View(QualifiedNameKey),
    MaterializedView(QualifiedNameKey),
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
