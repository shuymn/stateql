use stateql_core::{
    CommentTarget, Expr, Ident, IndexDef, IndexOwner, PrivilegeObject, QualifiedName, SchemaObject,
    Table, TypeDef, TypeKind, Value,
};

use crate::extra_keys;

pub(crate) fn normalize_object(object: &mut SchemaObject) {
    normalize_object_types(object);
    normalize_identifier_case(object);
    normalize_object_exprs(object);
}

fn normalize_object_types(object: &mut SchemaObject) {
    match object {
        SchemaObject::Table(table) => {
            for column in &mut table.columns {
                types::normalize_data_type(&mut column.data_type);
                if let Some(collation) = &mut column.collation {
                    *collation = collation.trim().to_ascii_lowercase();
                }
            }
            normalize_source_sql_hint(table);
        }
        SchemaObject::View(_) => {}
        SchemaObject::MaterializedView(materialized_view) => {
            for column in &mut materialized_view.columns {
                types::normalize_data_type(&mut column.data_type);
            }
        }
        SchemaObject::Index(_) => {}
        SchemaObject::Sequence(sequence) => {
            if let Some(data_type) = &mut sequence.data_type {
                types::normalize_data_type(data_type);
            }
        }
        SchemaObject::Trigger(_) => {}
        SchemaObject::Function(function) => {
            if let Some(return_type) = &mut function.return_type {
                types::normalize_data_type(return_type);
            }
            for param in &mut function.params {
                types::normalize_data_type(&mut param.data_type);
            }
        }
        SchemaObject::Type(type_def) => normalize_type_def(type_def),
        SchemaObject::Domain(domain) => types::normalize_data_type(&mut domain.data_type),
        SchemaObject::Extension(_) => {}
        SchemaObject::Schema(_) => {}
        SchemaObject::Comment(_) => {}
        SchemaObject::Privilege(_) => {}
        SchemaObject::Policy(_) => {}
    }
}

fn normalize_source_sql_hint(table: &mut Table) {
    if let Some(Value::String(source_sql)) =
        table.options.extra.get_mut(extra_keys::TABLE_SOURCE_SQL)
    {
        *source_sql = source_sql.trim().to_string();
    }
}

fn normalize_type_def(type_def: &mut TypeDef) {
    match &mut type_def.kind {
        TypeKind::Enum { labels } => {
            for label in labels {
                *label = label.trim().to_string();
            }
        }
        TypeKind::Composite { fields } => {
            for (name, data_type) in fields {
                normalize_ident(name);
                types::normalize_data_type(data_type);
            }
        }
        TypeKind::Range { subtype } => types::normalize_data_type(subtype),
    }
}

fn normalize_identifier_case(object: &mut SchemaObject) {
    match object {
        SchemaObject::Table(table) => {
            normalize_qualified_name(&mut table.name);
            if let Some(renamed_from) = &mut table.renamed_from {
                normalize_ident(renamed_from);
            }
            for column in &mut table.columns {
                normalize_ident(&mut column.name);
                if let Some(renamed_from) = &mut column.renamed_from {
                    normalize_ident(renamed_from);
                }
            }
            if let Some(primary_key) = &mut table.primary_key {
                if let Some(name) = &mut primary_key.name {
                    normalize_ident(name);
                }
                for column in &mut primary_key.columns {
                    normalize_ident(column);
                }
            }
            for foreign_key in &mut table.foreign_keys {
                if let Some(name) = &mut foreign_key.name {
                    normalize_ident(name);
                }
                for column in &mut foreign_key.columns {
                    normalize_ident(column);
                }
                normalize_qualified_name(&mut foreign_key.referenced_table);
                for column in &mut foreign_key.referenced_columns {
                    normalize_ident(column);
                }
            }
            for check in &mut table.checks {
                if let Some(name) = &mut check.name {
                    normalize_ident(name);
                }
            }
            for exclusion in &mut table.exclusions {
                if let Some(name) = &mut exclusion.name {
                    normalize_ident(name);
                }
            }
            if let Some(partition) = &mut table.partition {
                for column in &mut partition.columns {
                    normalize_ident(column);
                }
                for element in &mut partition.partitions {
                    normalize_ident(&mut element.name);
                }
            }
        }
        SchemaObject::View(view) => {
            normalize_qualified_name(&mut view.name);
            for column in &mut view.columns {
                normalize_ident(column);
            }
            if let Some(renamed_from) = &mut view.renamed_from {
                normalize_ident(renamed_from);
            }
        }
        SchemaObject::MaterializedView(view) => {
            normalize_qualified_name(&mut view.name);
            for column in &mut view.columns {
                normalize_ident(&mut column.name);
                if let Some(renamed_from) = &mut column.renamed_from {
                    normalize_ident(renamed_from);
                }
            }
            if let Some(renamed_from) = &mut view.renamed_from {
                normalize_ident(renamed_from);
            }
        }
        SchemaObject::Index(index) => {
            if let Some(name) = &mut index.name {
                normalize_ident(name);
            }
            normalize_index_owner(&mut index.owner);
        }
        SchemaObject::Sequence(sequence) => {
            normalize_qualified_name(&mut sequence.name);
            if let Some((table, column)) = &mut sequence.owned_by {
                normalize_qualified_name(table);
                normalize_ident(column);
            }
        }
        SchemaObject::Trigger(trigger) => {
            normalize_qualified_name(&mut trigger.name);
            normalize_qualified_name(&mut trigger.table);
        }
        SchemaObject::Function(function) => {
            normalize_qualified_name(&mut function.name);
            for param in &mut function.params {
                if let Some(name) = &mut param.name {
                    normalize_ident(name);
                }
            }
        }
        SchemaObject::Type(type_def) => normalize_qualified_name(&mut type_def.name),
        SchemaObject::Domain(domain) => normalize_qualified_name(&mut domain.name),
        SchemaObject::Extension(extension) => {
            normalize_ident(&mut extension.name);
            if let Some(schema) = &mut extension.schema {
                normalize_ident(schema);
            }
        }
        SchemaObject::Schema(schema) => normalize_ident(&mut schema.name),
        SchemaObject::Comment(comment) => normalize_comment_target(&mut comment.target),
        SchemaObject::Privilege(privilege) => {
            normalize_privilege_object(&mut privilege.on);
            normalize_ident(&mut privilege.grantee);
        }
        SchemaObject::Policy(policy) => {
            normalize_ident(&mut policy.name);
            normalize_qualified_name(&mut policy.table);
            for role in &mut policy.roles {
                normalize_ident(role);
            }
        }
    }
}

fn normalize_index_owner(owner: &mut IndexOwner) {
    match owner {
        IndexOwner::Table(name) | IndexOwner::View(name) | IndexOwner::MaterializedView(name) => {
            normalize_qualified_name(name);
        }
    }
}

fn normalize_comment_target(target: &mut CommentTarget) {
    match target {
        CommentTarget::Table(name)
        | CommentTarget::Index(name)
        | CommentTarget::View(name)
        | CommentTarget::MaterializedView(name)
        | CommentTarget::Sequence(name)
        | CommentTarget::Trigger(name)
        | CommentTarget::Function(name)
        | CommentTarget::Type(name)
        | CommentTarget::Domain(name) => normalize_qualified_name(name),
        CommentTarget::Column { table, column } => {
            normalize_qualified_name(table);
            normalize_ident(column);
        }
        CommentTarget::Extension(name) | CommentTarget::Schema(name) => normalize_ident(name),
    }
}

fn normalize_privilege_object(object: &mut PrivilegeObject) {
    match object {
        PrivilegeObject::Table(name)
        | PrivilegeObject::View(name)
        | PrivilegeObject::MaterializedView(name)
        | PrivilegeObject::Sequence(name)
        | PrivilegeObject::Domain(name)
        | PrivilegeObject::Type(name)
        | PrivilegeObject::Function(name) => normalize_qualified_name(name),
        PrivilegeObject::Schema(name) | PrivilegeObject::Database(name) => normalize_ident(name),
    }
}

fn normalize_qualified_name(name: &mut QualifiedName) {
    if let Some(schema) = &mut name.schema {
        normalize_ident(schema);
    }
    normalize_ident(&mut name.name);
}

fn normalize_ident(ident: &mut Ident) {
    ident.value = ident.value.to_ascii_lowercase();
    ident.quoted = false;
}

fn normalize_object_exprs(object: &mut SchemaObject) {
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
        }
        SchemaObject::View(view) => {
            view.query = view.query.trim().to_string();
        }
        SchemaObject::MaterializedView(materialized_view) => {
            materialized_view.query = materialized_view.query.trim().to_string();
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
            trigger.body = trigger.body.trim().to_string();
        }
        SchemaObject::Function(function) => {
            function.body = function.body.trim().to_string();
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

fn normalize_expr(expr: &mut Expr) {
    match expr {
        Expr::Literal(_) | Expr::Null => {}
        Expr::Ident(ident) => normalize_ident(ident),
        Expr::QualifiedIdent { qualifier, name } => {
            normalize_ident(qualifier);
            normalize_ident(name);
        }
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
        Expr::Exists(subquery) => {
            subquery.sql = subquery.sql.trim().to_string();
        }
    }
}

mod types {
    use stateql_core::DataType;

    pub(super) fn normalize_data_type(data_type: &mut DataType) {
        match data_type {
            DataType::Array(inner) => normalize_data_type(inner.as_mut()),
            DataType::Custom(custom) => {
                *data_type = normalize_custom_type(custom);
            }
            _ => {}
        }
    }

    fn normalize_custom_type(raw: &str) -> DataType {
        let canonical = normalize_custom(raw);
        if canonical.is_empty() {
            return DataType::Custom(String::new());
        }

        if canonical == "bit" || canonical == "bool" || canonical == "boolean" {
            return DataType::Boolean;
        }
        if canonical == "smallint" {
            return DataType::SmallInt;
        }
        if canonical == "int" || canonical == "integer" {
            return DataType::Integer;
        }
        if canonical == "bigint" {
            return DataType::BigInt;
        }
        if canonical == "real" {
            return DataType::Real;
        }
        if canonical == "float" {
            return DataType::DoublePrecision;
        }
        if canonical == "text" || canonical == "ntext" {
            return DataType::Text;
        }
        if canonical == "date" {
            return DataType::Date;
        }
        if canonical == "time" {
            return DataType::Time {
                with_timezone: false,
            };
        }
        if canonical == "datetime"
            || canonical == "datetime2"
            || canonical == "smalldatetime"
            || canonical == "datetimeoffset"
        {
            return DataType::Timestamp {
                with_timezone: false,
            };
        }
        if canonical == "uniqueidentifier" {
            return DataType::Uuid;
        }
        if canonical.starts_with("decimal") || canonical.starts_with("numeric") {
            return DataType::Numeric {
                precision: None,
                scale: None,
            };
        }
        if let Some(length) = parse_parenthesized_length(&canonical, "nvarchar") {
            return DataType::Varchar {
                length: Some(length),
            };
        }
        if canonical == "nvarchar(max)" || canonical == "varchar(max)" {
            return DataType::Text;
        }
        if let Some(length) = parse_parenthesized_length(&canonical, "varchar") {
            return DataType::Varchar {
                length: Some(length),
            };
        }
        if let Some(length) = parse_parenthesized_length(&canonical, "nchar") {
            return DataType::Char {
                length: Some(length),
            };
        }
        if let Some(length) = parse_parenthesized_length(&canonical, "char") {
            return DataType::Char {
                length: Some(length),
            };
        }
        if canonical == "varbinary(max)" || canonical == "binary(max)" || canonical == "image" {
            return DataType::Blob;
        }

        DataType::Custom(canonical)
    }

    fn normalize_custom(raw: &str) -> String {
        collapse_spaces(raw).to_ascii_lowercase()
    }

    fn parse_parenthesized_length(canonical: &str, prefix: &str) -> Option<u32> {
        let body = canonical.strip_prefix(prefix)?.trim();
        let inner = body.strip_prefix('(')?.strip_suffix(')')?;
        inner.trim().parse::<u32>().ok()
    }

    fn collapse_spaces(input: &str) -> String {
        input.split_whitespace().collect::<Vec<_>>().join(" ")
    }
}
