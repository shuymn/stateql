use std::fmt::Write as _;

use stateql_core::{
    BinaryOperator, CheckConstraint, CheckOption, ComparisonOp, DataType, Expr, ForeignKey,
    ForeignKeyAction, Function, FunctionParamMode, FunctionSecurity, GenerateError, Ident,
    IndexDef, IndexOwner, IsTest, Literal, SchemaObject, Trigger, TriggerEvent, TriggerForEach,
    TriggerTiming, UnaryOperator, Value, ViewSecurity,
};

use crate::extra_keys;

const TO_SQL_TARGET: &str = "dialect export renderer";
const UNSUPPORTED_TABLE_VARIANT: &str = "CreateTableUnsupportedShape";
const UNSUPPORTED_INDEX_VARIANT: &str = "CreateIndexUnsupportedShape";
const UNSUPPORTED_TRIGGER_VARIANT: &str = "CreateTriggerUnsupportedShape";
const UNSUPPORTED_FUNCTION_VARIANT: &str = "CreateFunctionUnsupportedShape";

pub(crate) fn render_object(
    dialect_name: &str,
    object: &SchemaObject,
) -> stateql_core::Result<String> {
    match object {
        SchemaObject::Table(table) => render_table(dialect_name, table),
        SchemaObject::View(view) => render_view(view),
        SchemaObject::Index(index) => render_index(dialect_name, index),
        SchemaObject::Trigger(trigger) => render_trigger(dialect_name, trigger),
        SchemaObject::Function(function) => render_function(dialect_name, function),
        SchemaObject::MaterializedView(_) => {
            unsupported_variant_error(dialect_name, "MaterializedView")
        }
        SchemaObject::Sequence(_) => unsupported_variant_error(dialect_name, "Sequence"),
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

    if !table.exclusions.is_empty() {
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
        if let Some(name) = &primary_key.name {
            definitions.push(format!(
                "CONSTRAINT {} PRIMARY KEY ({columns})",
                render_ident(name)
            ));
        } else {
            definitions.push(format!("PRIMARY KEY ({columns})"));
        }
    }

    for foreign_key in &table.foreign_keys {
        definitions.push(render_foreign_key(foreign_key));
    }

    for check in &table.checks {
        definitions.push(render_check(check));
    }

    sql.push_str(&definitions.join(", "));
    sql.push(')');

    if let Some(partition_hint) = partition_hint(table) {
        write!(sql, " {partition_hint}").expect("writing to String should not fail");
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

fn partition_hint(table: &stateql_core::Table) -> Option<&str> {
    match table.options.extra.get(extra_keys::TABLE_PARTITION_SQL) {
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
        && (identity.always
            || identity.start.is_some()
            || identity.increment.is_some()
            || identity.min_value.is_some()
            || identity.max_value.is_some())
    {
        sql.push_str(" AUTO_INCREMENT");
    }
    if column
        .extra
        .get(stateql_core::extra_keys::mysql::AUTO_INCREMENT)
        .is_some_and(|value| matches!(value, Value::Bool(true)))
    {
        sql.push_str(" AUTO_INCREMENT");
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
        write!(sql, " COLLATE {}", collation.trim()).expect("writing to String should not fail");
    }

    sql
}

fn render_foreign_key(foreign_key: &ForeignKey) -> String {
    let mut sql = String::new();
    if let Some(name) = &foreign_key.name {
        write!(sql, "CONSTRAINT {} ", render_ident(name))
            .expect("writing to String should not fail");
    }
    write!(
        sql,
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
    )
    .expect("writing to String should not fail");

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
    if let Some(name) = &check.name {
        format!(
            "CONSTRAINT {} CHECK ({})",
            render_ident(name),
            render_expr(&check.expr)
        )
    } else {
        format!("CHECK ({})", render_expr(&check.expr))
    }
}

fn render_view(view: &stateql_core::View) -> stateql_core::Result<String> {
    let mut sql = String::from("CREATE");
    if let Some(security) = view.security {
        write!(sql, " SQL SECURITY {}", render_view_security(security))
            .expect("writing to String should not fail");
    }
    write!(sql, " VIEW {}", render_qualified_name(&view.name))
        .expect("writing to String should not fail");

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
    if let Some(check_option) = view.check_option {
        write!(
            sql,
            " WITH {} CHECK OPTION",
            render_check_option(check_option)
        )
        .expect("writing to String should not fail");
    }
    sql.push(';');

    Ok(sql)
}

fn render_index(dialect_name: &str, index: &IndexDef) -> stateql_core::Result<String> {
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

    if let Some(method) = &index.method {
        write!(sql, " USING {}", method.trim().to_ascii_uppercase())
            .expect("writing to String should not fail");
    }
    if let Some(where_clause) = &index.where_clause {
        write!(sql, " WHERE {}", render_expr(where_clause))
            .expect("writing to String should not fail");
    }

    sql.push(';');
    Ok(sql)
}

fn render_trigger(dialect_name: &str, trigger: &Trigger) -> stateql_core::Result<String> {
    if trigger.events.is_empty() || trigger.for_each != TriggerForEach::Row {
        return unsupported_shape_error(dialect_name, UNSUPPORTED_TRIGGER_VARIANT);
    }
    if trigger.when_clause.is_some() {
        return unsupported_shape_error(dialect_name, UNSUPPORTED_TRIGGER_VARIANT);
    }

    let mut sql = format!(
        "CREATE TRIGGER {} {} {} ON {} FOR EACH ROW",
        render_qualified_name(&trigger.name),
        render_trigger_timing(trigger.timing),
        trigger
            .events
            .iter()
            .map(|event| render_trigger_event(*event))
            .collect::<Vec<_>>()
            .join(" OR "),
        render_qualified_name(&trigger.table),
    );

    let body = trigger.body.trim().trim_end_matches(';').trim();
    if body.is_empty() {
        return unsupported_shape_error(dialect_name, UNSUPPORTED_TRIGGER_VARIANT);
    }
    write!(sql, " {body}").expect("writing to String should not fail");
    sql.push(';');

    Ok(sql)
}

fn render_function(dialect_name: &str, function: &Function) -> stateql_core::Result<String> {
    let Some(return_type) = &function.return_type else {
        return unsupported_shape_error(dialect_name, UNSUPPORTED_FUNCTION_VARIANT);
    };

    let mut sql = format!(
        "CREATE FUNCTION {}({}) RETURNS {}",
        render_qualified_name(&function.name),
        function
            .params
            .iter()
            .map(render_function_param)
            .collect::<Vec<_>>()
            .join(", "),
        render_data_type(return_type)
    );

    if let Some(security) = function.security {
        write!(sql, " SQL SECURITY {}", render_function_security(security))
            .expect("writing to String should not fail");
    }

    let body = function.body.trim().trim_end_matches(';').trim();
    if body.is_empty() {
        return unsupported_shape_error(dialect_name, UNSUPPORTED_FUNCTION_VARIANT);
    }

    if body.to_ascii_uppercase().starts_with("BEGIN") {
        write!(sql, " {body}").expect("writing to String should not fail");
    } else {
        write!(sql, " RETURN {body}").expect("writing to String should not fail");
    }
    sql.push(';');

    Ok(sql)
}

fn render_function_param(param: &stateql_core::FunctionParam) -> String {
    let mut sql = String::new();
    if let Some(mode) = param.mode {
        write!(sql, "{} ", render_function_param_mode(mode))
            .expect("writing to String should not fail");
    }
    if let Some(name) = &param.name {
        write!(sql, "{} ", render_ident(name)).expect("writing to String should not fail");
    }
    sql.push_str(render_data_type(&param.data_type).as_str());
    if let Some(default) = &param.default {
        write!(sql, " DEFAULT {}", render_expr(default))
            .expect("writing to String should not fail");
    }
    sql
}

pub(crate) fn render_data_type(data_type: &DataType) -> String {
    match data_type {
        DataType::Boolean => "boolean".to_string(),
        DataType::SmallInt => "smallint".to_string(),
        DataType::Integer => "int".to_string(),
        DataType::BigInt => "bigint".to_string(),
        DataType::Real => "float".to_string(),
        DataType::DoublePrecision => "double".to_string(),
        DataType::Numeric { precision, scale } => match (precision, scale) {
            (Some(precision), Some(scale)) => format!("decimal({precision}, {scale})"),
            (Some(precision), None) => format!("decimal({precision})"),
            _ => "decimal".to_string(),
        },
        DataType::Text => "text".to_string(),
        DataType::Varchar { length } => match length {
            Some(length) => format!("varchar({length})"),
            None => "varchar(255)".to_string(),
        },
        DataType::Char { length } => match length {
            Some(length) => format!("char({length})"),
            None => "char(1)".to_string(),
        },
        DataType::Blob => "blob".to_string(),
        DataType::Date => "date".to_string(),
        DataType::Time { with_timezone: _ } => "time".to_string(),
        DataType::Timestamp { with_timezone: _ } => "timestamp".to_string(),
        DataType::Json | DataType::Jsonb => "json".to_string(),
        DataType::Uuid => "char(36)".to_string(),
        DataType::Array(inner) => format!("json /* array<{}> */", render_data_type(inner)),
        DataType::Custom(custom) => custom.trim().to_string(),
    }
}

pub(crate) fn render_expr(expr: &Expr) -> String {
    match expr {
        Expr::Literal(literal) => render_literal(literal),
        Expr::Ident(ident) => render_ident(ident),
        Expr::QualifiedIdent { qualifier, name } => {
            format!("{}.{}", render_ident(qualifier), render_ident(name))
        }
        Expr::Null => "NULL".to_string(),
        Expr::Raw(raw) => raw.trim().to_string(),
        Expr::BinaryOp { left, op, right } => format!(
            "{} {} {}",
            render_expr(left.as_ref()),
            render_binary_operator(op),
            render_expr(right.as_ref())
        ),
        Expr::UnaryOp { op, expr } => {
            let operand = render_expr(expr.as_ref());
            match op {
                UnaryOperator::Plus => format!("+{operand}"),
                UnaryOperator::Minus => format!("-{operand}"),
                UnaryOperator::Not => format!("NOT {operand}"),
            }
        }
        Expr::Comparison {
            left,
            op,
            right,
            quantifier,
        } => {
            let mut sql = format!(
                "{} {} {}",
                render_expr(left.as_ref()),
                render_comparison_op(op),
                render_expr(right.as_ref())
            );
            if let Some(quantifier) = quantifier {
                write!(sql, " {}", render_set_quantifier(quantifier))
                    .expect("writing to String should not fail");
            }
            sql
        }
        Expr::And(left, right) => {
            format!(
                "{} AND {}",
                render_expr(left.as_ref()),
                render_expr(right.as_ref())
            )
        }
        Expr::Or(left, right) => {
            format!(
                "{} OR {}",
                render_expr(left.as_ref()),
                render_expr(right.as_ref())
            )
        }
        Expr::Not(inner) => format!("NOT {}", render_expr(inner.as_ref())),
        Expr::Is { expr, test } => {
            format!("{} IS {}", render_expr(expr.as_ref()), render_is_test(test))
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
                render_expr(expr.as_ref()),
                not,
                render_expr(low.as_ref()),
                render_expr(high.as_ref())
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
                render_expr(expr.as_ref()),
                not,
                list.iter().map(render_expr).collect::<Vec<_>>().join(", ")
            )
        }
        Expr::Paren(inner) => format!("({})", render_expr(inner.as_ref())),
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
            let mut sql = String::new();
            write!(sql, "{}(", name.trim()).expect("writing to String should not fail");
            if *distinct {
                sql.push_str("DISTINCT ");
            }
            sql.push_str(
                args.iter()
                    .map(render_expr)
                    .collect::<Vec<_>>()
                    .join(", ")
                    .as_str(),
            );
            sql.push(')');
            if let Some(window_spec) = over {
                sql.push_str(" OVER (");
                if !window_spec.partition_by.is_empty() {
                    write!(
                        sql,
                        "PARTITION BY {}",
                        window_spec
                            .partition_by
                            .iter()
                            .map(render_expr)
                            .collect::<Vec<_>>()
                            .join(", ")
                    )
                    .expect("writing to String should not fail");
                }
                if !window_spec.order_by.is_empty() {
                    if !window_spec.partition_by.is_empty() {
                        sql.push(' ');
                    }
                    write!(
                        sql,
                        "ORDER BY {}",
                        window_spec
                            .order_by
                            .iter()
                            .map(render_expr)
                            .collect::<Vec<_>>()
                            .join(", ")
                    )
                    .expect("writing to String should not fail");
                }
                sql.push(')');
            }
            sql
        }
        Expr::Cast { expr, data_type } => {
            format!(
                "CAST({} AS {})",
                render_expr(expr.as_ref()),
                render_data_type(data_type)
            )
        }
        Expr::Collate { expr, collation } => {
            format!(
                "{} COLLATE {}",
                render_expr(expr.as_ref()),
                collation.trim()
            )
        }
        Expr::Case {
            operand,
            when_clauses,
            else_clause,
        } => {
            let mut sql = String::from("CASE");
            if let Some(operand) = operand {
                write!(sql, " {}", render_expr(operand.as_ref()))
                    .expect("writing to String should not fail");
            }
            for (when_expr, then_expr) in when_clauses {
                write!(
                    sql,
                    " WHEN {} THEN {}",
                    render_expr(when_expr),
                    render_expr(then_expr)
                )
                .expect("writing to String should not fail");
            }
            if let Some(else_expr) = else_clause {
                write!(sql, " ELSE {}", render_expr(else_expr.as_ref()))
                    .expect("writing to String should not fail");
            }
            sql.push_str(" END");
            sql
        }
        Expr::ArrayConstructor(items) => {
            format!(
                "JSON_ARRAY({})",
                items.iter().map(render_expr).collect::<Vec<_>>().join(", ")
            )
        }
        Expr::Exists(subquery) => format!("EXISTS ({})", subquery.sql.trim()),
    }
}

fn render_literal(literal: &Literal) -> String {
    match literal {
        Literal::String(value) => format!("'{}'", value.replace('\'', "''")),
        Literal::Integer(value) => value.to_string(),
        Literal::Float(value) => value.to_string(),
        Literal::Boolean(value) => {
            if *value {
                "TRUE".to_string()
            } else {
                "FALSE".to_string()
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
                "TRUE".to_string()
            } else {
                "FALSE".to_string()
            }
        }
        Value::Null => "NULL".to_string(),
    }
}

fn render_binary_operator(op: &BinaryOperator) -> &'static str {
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

fn render_comparison_op(op: &ComparisonOp) -> &'static str {
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

fn render_set_quantifier(quantifier: &stateql_core::SetQuantifier) -> &'static str {
    match quantifier {
        stateql_core::SetQuantifier::Any => "ANY",
        stateql_core::SetQuantifier::Some => "SOME",
        stateql_core::SetQuantifier::All => "ALL",
    }
}

fn render_is_test(test: &IsTest) -> &'static str {
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

fn render_trigger_timing(timing: TriggerTiming) -> &'static str {
    match timing {
        TriggerTiming::Before => "BEFORE",
        TriggerTiming::After => "AFTER",
        TriggerTiming::InsteadOf => "AFTER",
    }
}

fn render_trigger_event(event: TriggerEvent) -> &'static str {
    match event {
        TriggerEvent::Insert => "INSERT",
        TriggerEvent::Update => "UPDATE",
        TriggerEvent::Delete => "DELETE",
        TriggerEvent::Truncate => "DELETE",
    }
}

fn render_view_security(security: ViewSecurity) -> &'static str {
    match security {
        ViewSecurity::Definer => "DEFINER",
        ViewSecurity::Invoker => "INVOKER",
    }
}

fn render_check_option(check_option: CheckOption) -> &'static str {
    match check_option {
        CheckOption::Local => "LOCAL",
        CheckOption::Cascaded => "CASCADED",
    }
}

fn render_function_security(security: FunctionSecurity) -> &'static str {
    match security {
        FunctionSecurity::Definer => "DEFINER",
        FunctionSecurity::Invoker => "INVOKER",
    }
}

fn render_function_param_mode(mode: FunctionParamMode) -> &'static str {
    match mode {
        FunctionParamMode::In => "IN",
        FunctionParamMode::Out => "OUT",
        FunctionParamMode::InOut => "INOUT",
        FunctionParamMode::Variadic => "VARIADIC",
    }
}

pub(crate) fn render_qualified_name(name: &stateql_core::QualifiedName) -> String {
    if let Some(schema) = &name.schema {
        format!("{}.{}", render_ident(schema), render_ident(&name.name))
    } else {
        render_ident(&name.name)
    }
}

pub(crate) fn render_ident(ident: &Ident) -> String {
    format!("`{}`", ident.value.replace('`', "``"))
}

fn ensure_sql_terminated(sql: &str) -> String {
    let trimmed = sql.trim();
    if trimmed.ends_with(';') {
        trimmed.to_string()
    } else {
        format!("{trimmed};")
    }
}

fn unsupported_shape_error(dialect_name: &str, diff_op: &str) -> stateql_core::Result<String> {
    Err(GenerateError::UnsupportedDiffOp {
        diff_op: diff_op.to_string(),
        target: TO_SQL_TARGET.to_string(),
        dialect: dialect_name.to_string(),
    }
    .into())
}

fn unsupported_variant_error(dialect_name: &str, variant: &str) -> stateql_core::Result<String> {
    unsupported_shape_error(dialect_name, variant)
}
