use std::fmt::Write as _;

use stateql_core::{
    BinaryOperator, CheckConstraint, ComparisonOp, DataType, Expr, ForeignKey, ForeignKeyAction,
    GenerateError, Ident, IndexDef, IndexOwner, IsTest, Literal, SchemaObject, Trigger,
    TriggerEvent, TriggerForEach, TriggerTiming, UnaryOperator, Value,
};

use crate::extra_keys;

const TO_SQL_TARGET: &str = "dialect export renderer";
const UNSUPPORTED_TABLE_VARIANT: &str = "CreateTableUnsupportedShape";
const UNSUPPORTED_INDEX_VARIANT: &str = "CreateIndexUnsupportedShape";
const UNSUPPORTED_TRIGGER_VARIANT: &str = "CreateTriggerUnsupportedShape";

pub(crate) fn render_object(
    dialect_name: &str,
    object: &SchemaObject,
) -> stateql_core::Result<String> {
    match object {
        SchemaObject::Table(table) => render_table(dialect_name, table),
        SchemaObject::View(view) => render_view(view),
        SchemaObject::Index(index) => render_index(dialect_name, index),
        SchemaObject::Trigger(trigger) => render_trigger(dialect_name, trigger),
        SchemaObject::MaterializedView(_) => {
            unsupported_variant_error(dialect_name, "MaterializedView")
        }
        SchemaObject::Sequence(_) => unsupported_variant_error(dialect_name, "Sequence"),
        SchemaObject::Function(_) => unsupported_variant_error(dialect_name, "Function"),
        SchemaObject::Type(_) => unsupported_variant_error(dialect_name, "Type"),
        SchemaObject::Domain(_) => unsupported_variant_error(dialect_name, "Domain"),
        SchemaObject::Extension(_) => unsupported_variant_error(dialect_name, "Extension"),
        SchemaObject::Schema(_) => unsupported_variant_error(dialect_name, "Schema"),
        SchemaObject::Comment(_) => unsupported_variant_error(dialect_name, "Comment"),
        SchemaObject::Privilege(_) => unsupported_variant_error(dialect_name, "Privilege"),
        SchemaObject::Policy(_) => unsupported_variant_error(dialect_name, "Policy"),
    }
}

fn render_table(dialect_name: &str, table: &stateql_core::Table) -> stateql_core::Result<String> {
    if table.columns.is_empty()
        && table.primary_key.is_none()
        && table.foreign_keys.is_empty()
        && table.checks.is_empty()
    {
        if let Some(source_hint) = source_table_sql_hint(table) {
            return Ok(ensure_sql_terminated(source_hint));
        }
        return unsupported_shape_error(dialect_name, UNSUPPORTED_TABLE_VARIANT);
    }

    if !table.exclusions.is_empty() || table.partition.is_some() {
        return unsupported_shape_error(dialect_name, UNSUPPORTED_TABLE_VARIANT);
    }

    let mut sql = String::new();
    write!(sql, "CREATE TABLE {} (", render_qualified_name(&table.name))
        .expect("writing to String should not fail");

    let mut definitions = Vec::new();
    for column in &table.columns {
        definitions.push(render_column(column));
    }

    if let Some(primary_key) = &table.primary_key {
        let columns = primary_key
            .columns
            .iter()
            .map(render_ident)
            .collect::<Vec<_>>()
            .join(", ");
        definitions.push(format!("PRIMARY KEY ({columns})"));
    }

    for foreign_key in &table.foreign_keys {
        definitions.push(render_foreign_key(foreign_key));
    }

    for check in &table.checks {
        definitions.push(render_check(check));
    }

    sql.push_str(&definitions.join(", "));
    sql.push(')');

    if table
        .options
        .extra
        .get(extra_keys::TABLE_WITHOUT_ROWID)
        .is_some_and(|value| matches!(value, Value::Bool(true)))
    {
        sql.push_str(" WITHOUT ROWID");
    }
    if table
        .options
        .extra
        .get(extra_keys::TABLE_STRICT)
        .is_some_and(|value| matches!(value, Value::Bool(true)))
    {
        sql.push_str(" STRICT");
    }

    sql.push(';');
    Ok(sql)
}

fn source_table_sql_hint(table: &stateql_core::Table) -> Option<&str> {
    match table.options.extra.get(extra_keys::TABLE_SOURCE_SQL) {
        Some(Value::String(value)) if !value.trim().is_empty() => Some(value.as_str()),
        _ => None,
    }
}

fn render_column(column: &stateql_core::Column) -> String {
    let mut sql = format!(
        "{} {}",
        render_ident(&column.name),
        render_data_type(&column.data_type)
    );

    if column.not_null {
        sql.push_str(" NOT NULL");
    }
    if let Some(default) = &column.default {
        write!(sql, " DEFAULT {}", render_expr(default))
            .expect("writing to String should not fail");
    }
    if let Some(identity) = &column.identity
        && (identity.always || identity.start.is_some() || identity.increment.is_some())
    {
        sql.push_str(" GENERATED ALWAYS AS IDENTITY");
    }
    if let Some(generated) = &column.generated {
        let storage = if generated.stored {
            "STORED"
        } else {
            "VIRTUAL"
        };
        write!(
            sql,
            " GENERATED ALWAYS AS ({}) {storage}",
            render_expr(&generated.expr)
        )
        .expect("writing to String should not fail");
    }
    if let Some(collation) = &column.collation {
        write!(
            sql,
            " COLLATE {}",
            render_ident(&Ident::unquoted(collation.as_str()))
        )
        .expect("writing to String should not fail");
    }

    sql
}

fn render_foreign_key(foreign_key: &ForeignKey) -> String {
    let mut sql = format!(
        "FOREIGN KEY ({}) REFERENCES {} ({})",
        foreign_key
            .columns
            .iter()
            .map(render_ident)
            .collect::<Vec<_>>()
            .join(", "),
        render_qualified_name(&foreign_key.referenced_table),
        foreign_key
            .referenced_columns
            .iter()
            .map(render_ident)
            .collect::<Vec<_>>()
            .join(", ")
    );

    if let Some(on_delete) = foreign_key.on_delete {
        write!(sql, " ON DELETE {}", render_fk_action(on_delete))
            .expect("writing to String should not fail");
    }
    if let Some(on_update) = foreign_key.on_update {
        write!(sql, " ON UPDATE {}", render_fk_action(on_update))
            .expect("writing to String should not fail");
    }

    sql
}

fn render_check(check: &CheckConstraint) -> String {
    format!("CHECK ({})", render_expr(&check.expr))
}

fn render_view(view: &stateql_core::View) -> stateql_core::Result<String> {
    let mut sql = format!("CREATE VIEW {}", render_qualified_name(&view.name));
    if !view.columns.is_empty() {
        let columns = view
            .columns
            .iter()
            .map(render_ident)
            .collect::<Vec<_>>()
            .join(", ");
        write!(sql, " ({columns})").expect("writing to String should not fail");
    }
    write!(sql, " AS {}", view.query.trim()).expect("writing to String should not fail");
    sql.push(';');
    Ok(sql)
}

fn render_index(dialect_name: &str, index: &IndexDef) -> stateql_core::Result<String> {
    if index.concurrent {
        return unsupported_shape_error(dialect_name, UNSUPPORTED_INDEX_VARIANT);
    }

    let Some(name) = &index.name else {
        return unsupported_shape_error(dialect_name, UNSUPPORTED_INDEX_VARIANT);
    };

    let table_name = match &index.owner {
        IndexOwner::Table(table) => table,
        IndexOwner::View(_) | IndexOwner::MaterializedView(_) => {
            return unsupported_shape_error(dialect_name, UNSUPPORTED_INDEX_VARIANT);
        }
    };

    let mut sql = String::from("CREATE ");
    if index.unique {
        sql.push_str("UNIQUE ");
    }
    write!(
        sql,
        "INDEX {} ON {} ({})",
        render_ident(name),
        render_qualified_name(table_name),
        index
            .columns
            .iter()
            .map(|column| render_expr(&column.expr))
            .collect::<Vec<_>>()
            .join(", ")
    )
    .expect("writing to String should not fail");

    if let Some(where_clause) = &index.where_clause {
        write!(sql, " WHERE {}", render_expr(where_clause))
            .expect("writing to String should not fail");
    }

    sql.push(';');
    Ok(sql)
}

fn render_trigger(dialect_name: &str, trigger: &Trigger) -> stateql_core::Result<String> {
    if trigger.events.is_empty() {
        return unsupported_shape_error(dialect_name, UNSUPPORTED_TRIGGER_VARIANT);
    }

    if trigger.for_each != TriggerForEach::Row {
        return unsupported_shape_error(dialect_name, UNSUPPORTED_TRIGGER_VARIANT);
    }

    let mut sql = format!(
        "CREATE TRIGGER {} {} {} ON {}",
        render_qualified_name(&trigger.name),
        render_trigger_timing(trigger.timing),
        trigger
            .events
            .iter()
            .map(|event| render_trigger_event(*event))
            .collect::<Vec<_>>()
            .join(" OR "),
        render_qualified_name(&trigger.table)
    );

    if let Some(when_clause) = &trigger.when_clause {
        write!(sql, " WHEN {}", render_expr(when_clause))
            .expect("writing to String should not fail");
    }

    sql.push_str(" FOR EACH ROW ");
    let body = trigger.body.trim().trim_end_matches(';').trim();
    if body.is_empty() {
        return unsupported_shape_error(dialect_name, UNSUPPORTED_TRIGGER_VARIANT);
    }

    if body.to_ascii_uppercase().starts_with("BEGIN") {
        sql.push_str(body);
    } else {
        write!(sql, "BEGIN {body} END").expect("writing to String should not fail");
    }
    sql.push(';');

    Ok(sql)
}

fn render_trigger_timing(timing: TriggerTiming) -> &'static str {
    match timing {
        TriggerTiming::Before => "BEFORE",
        TriggerTiming::After => "AFTER",
        TriggerTiming::InsteadOf => "INSTEAD OF",
    }
}

fn render_trigger_event(event: TriggerEvent) -> &'static str {
    match event {
        TriggerEvent::Insert => "INSERT",
        TriggerEvent::Update => "UPDATE",
        TriggerEvent::Delete => "DELETE",
        TriggerEvent::Truncate => "TRUNCATE",
    }
}

fn render_data_type(data_type: &DataType) -> String {
    match data_type {
        DataType::Boolean => "INTEGER".to_string(),
        DataType::SmallInt | DataType::Integer | DataType::BigInt => "INTEGER".to_string(),
        DataType::Real | DataType::DoublePrecision => "REAL".to_string(),
        DataType::Numeric { precision, scale } => match (precision, scale) {
            (Some(precision), Some(scale)) => format!("NUMERIC({precision}, {scale})"),
            (Some(precision), None) => format!("NUMERIC({precision})"),
            _ => "NUMERIC".to_string(),
        },
        DataType::Text | DataType::Varchar { .. } | DataType::Char { .. } => "TEXT".to_string(),
        DataType::Blob => "BLOB".to_string(),
        DataType::Date | DataType::Time { .. } | DataType::Timestamp { .. } => "TEXT".to_string(),
        DataType::Json | DataType::Jsonb | DataType::Uuid => "TEXT".to_string(),
        DataType::Array(_) => "TEXT".to_string(),
        DataType::Custom(custom) => custom.trim().to_ascii_uppercase(),
    }
}

fn render_expr(expr: &Expr) -> String {
    match expr {
        Expr::Literal(literal) => render_literal(literal),
        Expr::Ident(ident) => render_ident(ident),
        Expr::QualifiedIdent { qualifier, name } => {
            format!("{}.{}", render_ident(qualifier), render_ident(name))
        }
        Expr::Null => "NULL".to_string(),
        Expr::Raw(raw) => raw.trim().to_string(),
        Expr::BinaryOp { left, op, right } => {
            format!(
                "{} {} {}",
                render_expr(left),
                render_binary_op(op.clone()),
                render_expr(right)
            )
        }
        Expr::UnaryOp { op, expr } => {
            format!("{}{}", render_unary_op(op.clone()), render_expr(expr))
        }
        Expr::Comparison {
            left,
            op,
            right,
            quantifier,
        } => {
            let mut rendered = format!(
                "{} {} {}",
                render_expr(left),
                render_comparison_op(op.clone()),
                render_expr(right)
            );
            if let Some(quantifier) = quantifier {
                write!(rendered, " {}", render_quantifier(quantifier.clone()))
                    .expect("writing to String should not fail");
            }
            rendered
        }
        Expr::And(left, right) => format!("{} AND {}", render_expr(left), render_expr(right)),
        Expr::Or(left, right) => format!("{} OR {}", render_expr(left), render_expr(right)),
        Expr::Not(inner) => format!("NOT {}", render_expr(inner)),
        Expr::Is { expr, test } => {
            format!("{} IS {}", render_expr(expr), render_is_test(test.clone()))
        }
        Expr::Between {
            expr,
            low,
            high,
            negated,
        } => {
            let not = if *negated { " NOT" } else { "" };
            format!(
                "{}{} BETWEEN {} AND {}",
                render_expr(expr),
                not,
                render_expr(low),
                render_expr(high)
            )
        }
        Expr::In {
            expr,
            list,
            negated,
        } => {
            let not = if *negated { " NOT" } else { "" };
            format!(
                "{}{} IN ({})",
                render_expr(expr),
                not,
                list.iter().map(render_expr).collect::<Vec<_>>().join(", ")
            )
        }
        Expr::Paren(inner) => format!("({})", render_expr(inner)),
        Expr::Tuple(items) => format!(
            "({})",
            items.iter().map(render_expr).collect::<Vec<_>>().join(", ")
        ),
        Expr::Function {
            name,
            args,
            distinct,
            over,
        } => {
            let mut rendered = String::new();
            write!(rendered, "{name}(").expect("writing to String should not fail");
            if *distinct {
                rendered.push_str("DISTINCT ");
            }
            rendered.push_str(&args.iter().map(render_expr).collect::<Vec<_>>().join(", "));
            rendered.push(')');
            if let Some(window) = over {
                rendered.push_str(" OVER (");
                let mut clauses = Vec::new();
                if !window.partition_by.is_empty() {
                    clauses.push(format!(
                        "PARTITION BY {}",
                        window
                            .partition_by
                            .iter()
                            .map(render_expr)
                            .collect::<Vec<_>>()
                            .join(", ")
                    ));
                }
                if !window.order_by.is_empty() {
                    clauses.push(format!(
                        "ORDER BY {}",
                        window
                            .order_by
                            .iter()
                            .map(render_expr)
                            .collect::<Vec<_>>()
                            .join(", ")
                    ));
                }
                rendered.push_str(&clauses.join(" "));
                rendered.push(')');
            }
            rendered
        }
        Expr::Cast { expr, data_type } => {
            format!(
                "CAST({} AS {})",
                render_expr(expr),
                render_data_type(data_type)
            )
        }
        Expr::Collate { expr, collation } => {
            format!("{} COLLATE {}", render_expr(expr), collation)
        }
        Expr::Case {
            operand,
            when_clauses,
            else_clause,
        } => {
            let mut rendered = String::from("CASE");
            if let Some(operand) = operand {
                write!(rendered, " {}", render_expr(operand))
                    .expect("writing to String should not fail");
            }
            for (when_expr, then_expr) in when_clauses {
                write!(
                    rendered,
                    " WHEN {} THEN {}",
                    render_expr(when_expr),
                    render_expr(then_expr)
                )
                .expect("writing to String should not fail");
            }
            if let Some(else_expr) = else_clause {
                write!(rendered, " ELSE {}", render_expr(else_expr))
                    .expect("writing to String should not fail");
            }
            rendered.push_str(" END");
            rendered
        }
        Expr::ArrayConstructor(items) => {
            format!(
                "({})",
                items.iter().map(render_expr).collect::<Vec<_>>().join(", ")
            )
        }
        Expr::Exists(sub_query) => format!("EXISTS ({})", sub_query.sql.trim()),
    }
}

fn render_literal(literal: &Literal) -> String {
    match literal {
        Literal::String(value) => format!("'{}'", value.replace('\'', "''")),
        Literal::Integer(value) => value.to_string(),
        Literal::Float(value) => value.to_string(),
        Literal::Boolean(value) => {
            if *value {
                "1".to_string()
            } else {
                "0".to_string()
            }
        }
        Literal::Value(value) => render_value(value),
    }
}

fn render_value(value: &Value) -> String {
    match value {
        Value::String(value) => format!("'{}'", value.replace('\'', "''")),
        Value::Integer(value) => value.to_string(),
        Value::Float(value) => value.to_string(),
        Value::Bool(value) => {
            if *value {
                "1".to_string()
            } else {
                "0".to_string()
            }
        }
        Value::Null => "NULL".to_string(),
    }
}

fn render_binary_op(op: BinaryOperator) -> &'static str {
    match op {
        BinaryOperator::Add => "+",
        BinaryOperator::Subtract => "-",
        BinaryOperator::Multiply => "*",
        BinaryOperator::Divide => "/",
        BinaryOperator::Modulo => "%",
        BinaryOperator::StringConcat => "||",
        BinaryOperator::BitwiseAnd => "&",
        BinaryOperator::BitwiseOr => "|",
        BinaryOperator::BitwiseXor => "^",
    }
}

fn render_unary_op(op: UnaryOperator) -> &'static str {
    match op {
        UnaryOperator::Plus => "+",
        UnaryOperator::Minus => "-",
        UnaryOperator::Not => "NOT ",
    }
}

fn render_comparison_op(op: ComparisonOp) -> &'static str {
    match op {
        ComparisonOp::Equal => "=",
        ComparisonOp::NotEqual => "!=",
        ComparisonOp::GreaterThan => ">",
        ComparisonOp::GreaterThanOrEqual => ">=",
        ComparisonOp::LessThan => "<",
        ComparisonOp::LessThanOrEqual => "<=",
        ComparisonOp::Like => "LIKE",
        ComparisonOp::ILike => "LIKE",
    }
}

fn render_quantifier(quantifier: stateql_core::SetQuantifier) -> &'static str {
    match quantifier {
        stateql_core::SetQuantifier::Any => "ANY",
        stateql_core::SetQuantifier::Some => "SOME",
        stateql_core::SetQuantifier::All => "ALL",
    }
}

fn render_is_test(test: IsTest) -> &'static str {
    match test {
        IsTest::Null => "NULL",
        IsTest::NotNull => "NOT NULL",
        IsTest::True => "TRUE",
        IsTest::NotTrue => "NOT TRUE",
        IsTest::False => "FALSE",
        IsTest::NotFalse => "NOT FALSE",
        IsTest::Unknown => "UNKNOWN",
        IsTest::NotUnknown => "NOT UNKNOWN",
    }
}

fn render_fk_action(action: ForeignKeyAction) -> &'static str {
    match action {
        ForeignKeyAction::NoAction => "NO ACTION",
        ForeignKeyAction::Restrict => "RESTRICT",
        ForeignKeyAction::Cascade => "CASCADE",
        ForeignKeyAction::SetNull => "SET NULL",
        ForeignKeyAction::SetDefault => "SET DEFAULT",
    }
}

fn render_qualified_name(name: &stateql_core::QualifiedName) -> String {
    match &name.schema {
        Some(schema) => format!("{}.{}", render_ident(schema), render_ident(&name.name)),
        None => render_ident(&name.name),
    }
}

fn render_ident(ident: &Ident) -> String {
    format!("\"{}\"", ident.value.replace('"', "\"\""))
}

fn ensure_sql_terminated(sql: &str) -> String {
    let trimmed = sql.trim();
    if trimmed.ends_with(';') {
        trimmed.to_string()
    } else {
        format!("{trimmed};")
    }
}

fn unsupported_variant_error(dialect_name: &str, variant: &str) -> stateql_core::Result<String> {
    Err(GenerateError::UnsupportedDiffOp {
        diff_op: format!("ToSql::{variant}"),
        target: TO_SQL_TARGET.to_string(),
        dialect: dialect_name.to_string(),
    }
    .into())
}

fn unsupported_shape_error(dialect_name: &str, variant: &str) -> stateql_core::Result<String> {
    unsupported_variant_error(dialect_name, variant)
}
