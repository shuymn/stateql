use stateql_core::{
    DataType, Expr, Function, Ident, IndexDef, MaterializedView, SchemaObject, Sequence, Table,
    TypeDef, TypeKind,
};

use crate::extra_keys;

pub(crate) fn normalize_object(object: &mut SchemaObject) {
    normalize_object_types(object);
    normalize_object_exprs(object);
}

fn normalize_object_types(object: &mut SchemaObject) {
    match object {
        SchemaObject::Table(table) => {
            for column in &mut table.columns {
                normalize_data_type(&mut column.data_type);
                if let Some(collation) = &mut column.collation {
                    *collation = collation.trim().to_ascii_lowercase();
                }
            }
            normalize_source_sql_hint(table);
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

fn normalize_source_sql_hint(table: &mut Table) {
    if let Some(stateql_core::Value::String(source_sql)) =
        table.options.extra.get_mut(extra_keys::TABLE_SOURCE_SQL)
    {
        *source_sql = source_sql.trim().to_string();
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

fn normalize_data_type(data_type: &mut DataType) {
    match data_type {
        DataType::Array(inner) => normalize_data_type(inner.as_mut()),
        DataType::Custom(custom) => {
            *data_type = normalize_custom_type(custom);
        }
        _ => {}
    }
}

fn normalize_custom_type(raw: &str) -> DataType {
    let canonical = collapse_spaces(raw).to_ascii_lowercase();

    if canonical.is_empty() {
        return DataType::Custom(String::new());
    }

    if is_integer_affinity(&canonical) {
        return DataType::Integer;
    }
    if is_text_affinity(&canonical) {
        return DataType::Text;
    }
    if canonical.contains("blob") {
        return DataType::Blob;
    }
    if canonical.contains("real") || canonical.contains("floa") || canonical.contains("doub") {
        return DataType::Real;
    }
    if canonical.contains("numeric")
        || canonical.contains("decimal")
        || canonical.contains("bool")
        || canonical.contains("date")
        || canonical.contains("time")
    {
        return DataType::Numeric {
            precision: None,
            scale: None,
        };
    }

    DataType::Custom(canonical)
}

fn is_integer_affinity(canonical: &str) -> bool {
    canonical.contains("int")
}

fn is_text_affinity(canonical: &str) -> bool {
    canonical.contains("char") || canonical.contains("clob") || canonical.contains("text")
}

fn collapse_spaces(input: &str) -> String {
    input.split_whitespace().collect::<Vec<_>>().join(" ")
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
            for column in &mut view.columns {
                normalize_ident(column);
            }
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

fn normalize_ident(ident: &mut Ident) {
    if !ident.quoted {
        ident.value = ident.value.to_ascii_lowercase();
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
