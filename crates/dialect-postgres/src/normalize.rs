use stateql_core::{
    Column, DataType, Expr, Function, Ident, Identity, IndexDef, MaterializedView, Partition,
    PartitionBound, PartitionElement, QualifiedName, SchemaObject, Sequence, Table, TypeDef,
    TypeKind, Value,
};

use crate::extra_keys;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct QualifiedNameKey {
    schema: Option<String>,
    name: String,
}

pub(crate) fn normalize_object(object: &mut SchemaObject) {
    types::normalize_object_types(object);
    expr::normalize_object_exprs(object);
    sequence::normalize_object_sequences(object);
}

pub(crate) fn normalize_schema(objects: &mut Vec<SchemaObject>) {
    for object in objects.iter_mut() {
        normalize_object(object);
    }

    sequence::apply_sequence_contract(objects);
    partition::fold_partition_children(objects);
}

fn qualified_name_key(name: &QualifiedName) -> QualifiedNameKey {
    QualifiedNameKey {
        schema: name.schema.as_ref().map(ident_key),
        name: ident_key(&name.name),
    }
}

fn ident_key(ident: &Ident) -> String {
    if ident.quoted {
        format!("q:{}", ident.value)
    } else {
        format!("u:{}", ident.value.to_ascii_lowercase())
    }
}

mod types {
    use super::{DataType, Function, MaterializedView, SchemaObject, Sequence, TypeDef, TypeKind};

    pub(super) fn normalize_object_types(object: &mut SchemaObject) {
        match object {
            SchemaObject::Table(table) => {
                for column in &mut table.columns {
                    normalize_data_type(&mut column.data_type);
                }
            }
            SchemaObject::View(_) => {}
            SchemaObject::MaterializedView(materialized_view) => {
                normalize_materialized_view_types(materialized_view);
            }
            SchemaObject::Index(_) => {}
            SchemaObject::Sequence(sequence) => normalize_sequence_type(sequence),
            SchemaObject::Trigger(_) => {}
            SchemaObject::Function(function) => normalize_function_types(function),
            SchemaObject::Type(type_def) => normalize_type_def(type_def),
            SchemaObject::Domain(domain) => normalize_data_type(&mut domain.data_type),
            SchemaObject::Extension(_) => {}
            SchemaObject::Schema(_) => {}
            SchemaObject::Comment(_) => {}
            SchemaObject::Privilege(_) => {}
            SchemaObject::Policy(_) => {}
        }
    }

    fn normalize_materialized_view_types(materialized_view: &mut MaterializedView) {
        for column in &mut materialized_view.columns {
            normalize_data_type(&mut column.data_type);
        }
    }

    fn normalize_sequence_type(sequence: &mut Sequence) {
        if let Some(data_type) = &mut sequence.data_type {
            normalize_data_type(data_type);
        }
    }

    fn normalize_function_types(function: &mut Function) {
        if let Some(return_type) = &mut function.return_type {
            normalize_data_type(return_type);
        }

        for param in &mut function.params {
            normalize_data_type(&mut param.data_type);
        }
    }

    fn normalize_type_def(type_def: &mut TypeDef) {
        match &mut type_def.kind {
            TypeKind::Enum { .. } => {}
            TypeKind::Composite { fields } => {
                for (_, data_type) in fields {
                    normalize_data_type(data_type);
                }
            }
            TypeKind::Range { subtype } => normalize_data_type(subtype),
        }
    }

    pub(super) fn normalize_data_type(data_type: &mut DataType) {
        match data_type {
            DataType::Array(inner) => normalize_data_type(inner.as_mut()),
            DataType::Custom(custom) => {
                *data_type = normalize_custom_data_type(custom);
            }
            _ => {}
        }
    }

    fn normalize_custom_data_type(raw: &str) -> DataType {
        let canonical = canonical_custom(raw);
        let base = canonical
            .rsplit('.')
            .next()
            .map(str::trim)
            .unwrap_or_default();

        if let Some(alias) = alias_to_data_type(base) {
            alias
        } else {
            DataType::Custom(canonical)
        }
    }

    fn alias_to_data_type(base: &str) -> Option<DataType> {
        match base {
            "bool" | "boolean" => Some(DataType::Boolean),
            "int2" | "smallint" => Some(DataType::SmallInt),
            "int" | "int4" | "integer" => Some(DataType::Integer),
            "int8" | "bigint" => Some(DataType::BigInt),
            "float4" | "real" => Some(DataType::Real),
            "float8" | "double" | "double precision" => Some(DataType::DoublePrecision),
            "numeric" | "decimal" => Some(DataType::Numeric {
                precision: None,
                scale: None,
            }),
            "text" => Some(DataType::Text),
            "varchar" | "character varying" => Some(DataType::Varchar { length: None }),
            "bpchar" | "char" | "character" => Some(DataType::Char { length: None }),
            "bytea" => Some(DataType::Blob),
            "date" => Some(DataType::Date),
            "time" | "time without time zone" => Some(DataType::Time {
                with_timezone: false,
            }),
            "timetz" | "time with time zone" => Some(DataType::Time {
                with_timezone: true,
            }),
            "timestamp" | "timestamp without time zone" => Some(DataType::Timestamp {
                with_timezone: false,
            }),
            "timestamptz" | "timestamp with time zone" => Some(DataType::Timestamp {
                with_timezone: true,
            }),
            "json" => Some(DataType::Json),
            "jsonb" => Some(DataType::Jsonb),
            "uuid" => Some(DataType::Uuid),
            _ => None,
        }
    }

    fn canonical_custom(raw: &str) -> String {
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            return String::new();
        }

        split_qualified_segments(trimmed)
            .into_iter()
            .map(|segment| normalize_segment(&segment))
            .collect::<Vec<_>>()
            .join(".")
    }

    fn split_qualified_segments(input: &str) -> Vec<String> {
        let mut segments = Vec::new();
        let mut current = String::new();
        let mut in_double_quote = false;

        for ch in input.chars() {
            match ch {
                '"' => {
                    in_double_quote = !in_double_quote;
                    current.push(ch);
                }
                '.' if !in_double_quote => {
                    segments.push(current);
                    current = String::new();
                }
                _ => current.push(ch),
            }
        }

        segments.push(current);
        segments
    }

    fn normalize_segment(segment: &str) -> String {
        let trimmed = segment.trim();
        if trimmed.len() >= 2 && trimmed.starts_with('"') && trimmed.ends_with('"') {
            trimmed[1..trimmed.len() - 1].to_ascii_lowercase()
        } else {
            collapse_spaces(trimmed).to_ascii_lowercase()
        }
    }

    fn collapse_spaces(input: &str) -> String {
        input.split_whitespace().collect::<Vec<_>>().join(" ")
    }
}

mod expr {
    use super::{Expr, IndexDef, Partition, PartitionBound, SchemaObject};

    pub(super) fn normalize_object_exprs(object: &mut SchemaObject) {
        match object {
            SchemaObject::Table(table) => {
                for column in &mut table.columns {
                    if let Some(default) = &mut column.default {
                        normalize_expr(default);
                    }
                    if let Some(generated) = &mut column.generated {
                        normalize_expr(&mut generated.expr);
                    }
                }

                for check in &mut table.checks {
                    normalize_expr(&mut check.expr);
                }

                for exclusion in &mut table.exclusions {
                    for element in &mut exclusion.elements {
                        normalize_expr(&mut element.expr);
                    }
                    if let Some(where_clause) = &mut exclusion.where_clause {
                        normalize_expr(where_clause);
                    }
                }

                if let Some(partition) = &mut table.partition {
                    normalize_partition_bounds(partition);
                }
            }
            SchemaObject::View(_) => {}
            SchemaObject::MaterializedView(materialized_view) => {
                for column in &mut materialized_view.columns {
                    if let Some(default) = &mut column.default {
                        normalize_expr(default);
                    }
                    if let Some(generated) = &mut column.generated {
                        normalize_expr(&mut generated.expr);
                    }
                }
            }
            SchemaObject::Index(index) => normalize_index_exprs(index),
            SchemaObject::Sequence(_) => {}
            SchemaObject::Trigger(trigger) => {
                if let Some(when_clause) = &mut trigger.when_clause {
                    normalize_expr(when_clause);
                }
            }
            SchemaObject::Function(function) => {
                for param in &mut function.params {
                    if let Some(default) = &mut param.default {
                        normalize_expr(default);
                    }
                }
            }
            SchemaObject::Type(_) => {}
            SchemaObject::Domain(domain) => {
                if let Some(default) = &mut domain.default {
                    normalize_expr(default);
                }
                for check in &mut domain.checks {
                    normalize_expr(&mut check.expr);
                }
            }
            SchemaObject::Extension(_) => {}
            SchemaObject::Schema(_) => {}
            SchemaObject::Comment(_) => {}
            SchemaObject::Privilege(_) => {}
            SchemaObject::Policy(policy) => {
                if let Some(using_expr) = &mut policy.using_expr {
                    normalize_expr(using_expr);
                }
                if let Some(check_expr) = &mut policy.check_expr {
                    normalize_expr(check_expr);
                }
            }
        }
    }

    fn normalize_index_exprs(index: &mut IndexDef) {
        for column in &mut index.columns {
            normalize_expr(&mut column.expr);
        }

        if let Some(where_clause) = &mut index.where_clause {
            normalize_expr(where_clause);
        }
    }

    fn normalize_partition_bounds(partition: &mut Partition) {
        for element in &mut partition.partitions {
            let Some(bound) = &mut element.bound else {
                continue;
            };

            match bound {
                PartitionBound::LessThan(exprs) | PartitionBound::In(exprs) => {
                    for expr in exprs {
                        normalize_expr(expr);
                    }
                }
                PartitionBound::FromTo { from, to } => {
                    for expr in from {
                        normalize_expr(expr);
                    }
                    for expr in to {
                        normalize_expr(expr);
                    }
                }
                PartitionBound::MaxValue => {}
            }
        }
    }

    fn normalize_expr(expr: &mut Expr) {
        match expr {
            Expr::Literal(_) | Expr::Ident(_) | Expr::QualifiedIdent { .. } | Expr::Null => {}
            Expr::Raw(raw) => {
                *raw = raw.trim().to_string();
            }
            Expr::BinaryOp { left, right, .. } => {
                normalize_expr(left.as_mut());
                normalize_expr(right.as_mut());
            }
            Expr::UnaryOp { expr, .. } => normalize_expr(expr.as_mut()),
            Expr::Comparison {
                left,
                right,
                quantifier: _,
                ..
            } => {
                normalize_expr(left.as_mut());
                normalize_expr(right.as_mut());
            }
            Expr::And(left, right) | Expr::Or(left, right) => {
                normalize_expr(left.as_mut());
                normalize_expr(right.as_mut());
            }
            Expr::Not(inner) | Expr::Paren(inner) => normalize_expr(inner.as_mut()),
            Expr::Is { expr, .. } => normalize_expr(expr.as_mut()),
            Expr::Between {
                expr,
                low,
                high,
                negated: _,
            } => {
                normalize_expr(expr.as_mut());
                normalize_expr(low.as_mut());
                normalize_expr(high.as_mut());
            }
            Expr::In {
                expr,
                list,
                negated: _,
            } => {
                normalize_expr(expr.as_mut());
                for item in list {
                    normalize_expr(item);
                }
            }
            Expr::Tuple(items) | Expr::ArrayConstructor(items) => {
                for item in items {
                    normalize_expr(item);
                }
            }
            Expr::Function {
                args,
                over,
                distinct: _,
                name: _,
            } => {
                for arg in args {
                    normalize_expr(arg);
                }
                if let Some(window_spec) = over {
                    for expr in &mut window_spec.partition_by {
                        normalize_expr(expr);
                    }
                    for expr in &mut window_spec.order_by {
                        normalize_expr(expr);
                    }
                }
            }
            Expr::Cast { expr, .. } => normalize_expr(expr.as_mut()),
            Expr::Collate { expr, .. } => normalize_expr(expr.as_mut()),
            Expr::Case {
                operand,
                when_clauses,
                else_clause,
            } => {
                if let Some(operand) = operand {
                    normalize_expr(operand.as_mut());
                }
                for (when_expr, then_expr) in when_clauses {
                    normalize_expr(when_expr);
                    normalize_expr(then_expr);
                }
                if let Some(else_expr) = else_clause {
                    normalize_expr(else_expr.as_mut());
                }
            }
            Expr::Exists(sub_query) => {
                sub_query.sql = sub_query.sql.trim().to_string();
            }
        }
    }
}

mod sequence {
    use std::collections::{BTreeMap, BTreeSet};

    use super::{
        Column, DataType, Expr, Ident, Identity, QualifiedName, QualifiedNameKey, SchemaObject,
        Sequence, Table, Value, extra_keys, ident_key, qualified_name_key,
    };

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    enum ImplicitSequenceKind {
        Serial,
        Identity,
    }

    #[derive(Debug, Clone)]
    struct ImplicitSequenceTarget {
        table_index: usize,
        column_index: usize,
        table_name: QualifiedName,
        column_name: Ident,
        kind: ImplicitSequenceKind,
    }

    pub(super) fn normalize_object_sequences(object: &mut SchemaObject) {
        match object {
            SchemaObject::Table(table) => normalize_table_sequence_representation(table),
            SchemaObject::Sequence(_) => {}
            _ => {}
        }
    }

    pub(super) fn apply_sequence_contract(objects: &mut Vec<SchemaObject>) {
        let implicit_targets = collect_implicit_targets(objects);
        if implicit_targets.is_empty() {
            return;
        }

        let mut removals = BTreeSet::new();
        let mut identity_folds = Vec::new();

        for (index, object) in objects.iter().enumerate() {
            let SchemaObject::Sequence(sequence) = object else {
                continue;
            };

            let key = qualified_name_key(&sequence.name);
            let Some(target) = implicit_targets.get(&key) else {
                continue;
            };

            if !sequence_owned_by_target(sequence, &target.table_name, &target.column_name) {
                continue;
            }

            removals.insert(index);

            if target.kind == ImplicitSequenceKind::Identity {
                identity_folds.push((target.table_index, target.column_index, sequence.clone()));
            }
        }

        for (table_index, column_index, sequence) in identity_folds {
            let Some(SchemaObject::Table(table)) = objects.get_mut(table_index) else {
                continue;
            };
            let Some(column) = table.columns.get_mut(column_index) else {
                continue;
            };
            fold_identity_sequence_options(column, &sequence);
        }

        for index in removals.into_iter().rev() {
            objects.remove(index);
        }
    }

    fn collect_implicit_targets(
        objects: &[SchemaObject],
    ) -> BTreeMap<QualifiedNameKey, ImplicitSequenceTarget> {
        let mut targets = BTreeMap::new();

        for (table_index, object) in objects.iter().enumerate() {
            let SchemaObject::Table(table) = object else {
                continue;
            };

            for (column_index, column) in table.columns.iter().enumerate() {
                let implicit_name = implicit_sequence_name(&table.name, &column.name);
                let key = qualified_name_key(&implicit_name);
                let target = if column.identity.is_some() {
                    Some(ImplicitSequenceTarget {
                        table_index,
                        column_index,
                        table_name: table.name.clone(),
                        column_name: column.name.clone(),
                        kind: ImplicitSequenceKind::Identity,
                    })
                } else if column_uses_sequence(column, &implicit_name) {
                    Some(ImplicitSequenceTarget {
                        table_index,
                        column_index,
                        table_name: table.name.clone(),
                        column_name: column.name.clone(),
                        kind: ImplicitSequenceKind::Serial,
                    })
                } else {
                    None
                };

                if let Some(target) = target {
                    targets.insert(key, target);
                }
            }
        }

        targets
    }

    fn normalize_table_sequence_representation(table: &mut Table) {
        let table_name = table.name.clone();
        for column in &mut table.columns {
            hydrate_identity_from_extra(column);
            normalize_serial_alias(&table_name, column);

            if column.identity.is_some() {
                column.default = None;
            }
        }
    }

    fn hydrate_identity_from_extra(column: &mut Column) {
        if column.identity.is_some() {
            return;
        }

        let Some(Value::String(mode)) = column.extra.get(extra_keys::COLUMN_IDENTITY).cloned()
        else {
            return;
        };
        column.extra.remove(extra_keys::COLUMN_IDENTITY);
        column.identity = Some(identity_from_mode(&mode));
    }

    fn normalize_serial_alias(table_name: &QualifiedName, column: &mut Column) {
        let DataType::Custom(custom) = &column.data_type else {
            return;
        };

        let alias = custom.rsplit('.').next().unwrap_or(custom.as_str());
        let mapped_type = match alias {
            "serial" | "serial4" => Some(DataType::Integer),
            "bigserial" | "serial8" => Some(DataType::BigInt),
            "smallserial" | "serial2" => Some(DataType::SmallInt),
            _ => None,
        };

        let Some(mapped_type) = mapped_type else {
            return;
        };

        column.data_type = mapped_type;
        if column.default.is_none() {
            let implicit_sequence = implicit_sequence_name(table_name, &column.name);
            column.default = Some(Expr::Raw(nextval_expr(&implicit_sequence)));
        }
    }

    fn fold_identity_sequence_options(column: &mut Column, sequence: &Sequence) {
        let Some(identity) = &mut column.identity else {
            return;
        };

        if identity.start.is_none() {
            identity.start = sequence.start;
        }
        if identity.increment.is_none() {
            identity.increment = sequence.increment;
        }
        if identity.min_value.is_none() {
            identity.min_value = sequence.min_value;
        }
        if identity.max_value.is_none() {
            identity.max_value = sequence.max_value;
        }
        if identity.cache.is_none() {
            identity.cache = sequence.cache;
        }
        if !identity.cycle {
            identity.cycle = sequence.cycle;
        }
        column.default = None;
    }

    fn column_uses_sequence(column: &Column, sequence_name: &QualifiedName) -> bool {
        let Some(Expr::Raw(raw)) = &column.default else {
            return false;
        };

        let identifier = render_regclass_identifier(sequence_name);
        raw.contains("nextval(") && raw.contains(&identifier)
    }

    fn sequence_owned_by_target(
        sequence: &Sequence,
        table_name: &QualifiedName,
        column_name: &Ident,
    ) -> bool {
        let Some((owned_table, owned_column)) = &sequence.owned_by else {
            return false;
        };

        qualified_name_key(owned_table) == qualified_name_key(table_name)
            && ident_key(owned_column) == ident_key(column_name)
    }

    fn nextval_expr(sequence_name: &QualifiedName) -> String {
        let regclass = render_regclass_identifier(sequence_name).replace('\'', "''");
        format!("nextval('{regclass}'::regclass)")
    }

    fn render_regclass_identifier(name: &QualifiedName) -> String {
        match &name.schema {
            Some(schema) => format!("{}.{}", schema.value, name.name.value),
            None => name.name.value.clone(),
        }
    }

    fn implicit_sequence_name(table: &QualifiedName, column: &Ident) -> QualifiedName {
        QualifiedName {
            schema: table.schema.clone(),
            name: Ident::unquoted(format!("{}_{}_seq", table.name.value, column.value)),
        }
    }

    fn identity_from_mode(mode: &str) -> Identity {
        let normalized = mode.trim().to_ascii_lowercase();
        let always = matches!(normalized.as_str(), "a" | "always");
        Identity {
            always,
            start: None,
            increment: None,
            min_value: None,
            max_value: None,
            cache: None,
            cycle: false,
        }
    }
}

mod partition {
    use std::collections::BTreeSet;

    use super::{
        Ident, Partition, PartitionElement, QualifiedName, SchemaObject, Table, Value, extra_keys,
        qualified_name_key,
    };

    struct FoldOperation {
        child_index: usize,
        parent_index: usize,
        strategy: stateql_core::PartitionStrategy,
        element: PartitionElement,
    }

    pub(super) fn fold_partition_children(objects: &mut Vec<SchemaObject>) {
        let mut parent_index_by_name = std::collections::BTreeMap::new();

        for (index, object) in objects.iter().enumerate() {
            let SchemaObject::Table(table) = object else {
                continue;
            };
            if parent_of_child(table).is_some() {
                continue;
            }
            parent_index_by_name.insert(qualified_name_key(&table.name), index);
        }

        let mut fold_ops = Vec::new();
        for (index, object) in objects.iter().enumerate() {
            let SchemaObject::Table(child_table) = object else {
                continue;
            };
            let Some(parent_name) = parent_of_child(child_table) else {
                continue;
            };
            let Some(&parent_index) = parent_index_by_name.get(&qualified_name_key(&parent_name))
            else {
                continue;
            };
            if parent_index == index {
                continue;
            }

            let (strategy, element) = partition_element_from_child(child_table);
            fold_ops.push(FoldOperation {
                child_index: index,
                parent_index,
                strategy,
                element,
            });
        }

        let mut removal_indexes = BTreeSet::new();
        for fold in fold_ops {
            let Some(SchemaObject::Table(parent_table)) = objects.get_mut(fold.parent_index) else {
                continue;
            };

            let partition = parent_table.partition.get_or_insert_with(|| Partition {
                strategy: fold.strategy.clone(),
                columns: Vec::new(),
                partitions: Vec::new(),
            });

            if partition
                .partitions
                .iter()
                .any(|existing| existing.name == fold.element.name)
            {
                continue;
            }

            partition.partitions.push(fold.element);
            removal_indexes.insert(fold.child_index);
        }

        for index in removal_indexes.into_iter().rev() {
            objects.remove(index);
        }
    }

    fn parent_of_child(table: &Table) -> Option<QualifiedName> {
        let parent_name = table
            .options
            .extra
            .get(extra_keys::TABLE_PARTITION_PARENT_NAME)?;
        let Value::String(parent_name) = parent_name else {
            return None;
        };

        let schema = match table
            .options
            .extra
            .get(extra_keys::TABLE_PARTITION_PARENT_SCHEMA)
        {
            Some(Value::String(schema)) if !schema.is_empty() => Some(Ident::unquoted(schema)),
            _ => None,
        };

        Some(QualifiedName {
            schema,
            name: Ident::unquoted(parent_name),
        })
    }

    fn partition_element_from_child(
        child_table: &Table,
    ) -> (stateql_core::PartitionStrategy, PartitionElement) {
        if let Some(partition) = &child_table.partition
            && let Some(first) = partition.partitions.first()
        {
            return (partition.strategy.clone(), first.clone());
        }

        (
            stateql_core::PartitionStrategy::Range,
            PartitionElement {
                name: child_table.name.name.clone(),
                bound: None,
                extra: std::collections::BTreeMap::new(),
            },
        )
    }
}
