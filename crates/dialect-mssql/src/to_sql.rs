use std::fmt::Write as _;

use stateql_core::{
    BinaryOperator, CheckConstraint, ComparisonOp, DataType, Expr, ForeignKey, ForeignKeyAction,
    Function, FunctionParamMode, GenerateError, Ident, IndexDef, IndexOwner, IsTest, Literal,
    SchemaDef, SchemaObject, Trigger, TriggerEvent, TriggerTiming, UnaryOperator, Value,
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
        SchemaObject::Schema(schema) => render_schema(schema),
        SchemaObject::MaterializedView(_) => {
            unsupported_variant_error(dialect_name, "MaterializedView")
        }
        SchemaObject::Sequence(_) => unsupported_variant_error(dialect_name, "Sequence"),
        SchemaObject::Type(_) => unsupported_variant_error(dialect_name, "Type"),
        SchemaObject::Domain(_) => unsupported_variant_error(dialect_name, "Domain"),
        SchemaObject::Extension(_) => unsupported_variant_error(dialect_name, "Extension"),
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
        definitions.push(render_column(dialect_name, column)?);
    }

    if let Some(primary_key) = &table.primary_key {
        let mut pk_sql = String::new();
        if let Some(name) = &primary_key.name {
            write!(pk_sql, "CONSTRAINT {} ", render_ident(name))
                .expect("writing to String should not fail");
        }
        pk_sql.push_str("PRIMARY KEY");

        if let Some(Value::Bool(clustered)) = table
            .options
            .extra
            .get(extra_keys::TABLE_PRIMARY_KEY_CLUSTERED)
        {
            if *clustered {
                pk_sql.push_str(" CLUSTERED");
            } else {
                pk_sql.push_str(" NONCLUSTERED");
            }
        }

        let columns = primary_key
            .columns
            .iter()
            .map(render_ident)
            .collect::<Vec<_>>()
            .join(", ");
        write!(pk_sql, " ({columns})").expect("writing to String should not fail");
        definitions.push(pk_sql);
    }

    for foreign_key in &table.foreign_keys {
        definitions.push(render_foreign_key(foreign_key));
    }

    for check in &table.checks {
        definitions.push(render_check(check));
    }

    sql.push_str(&definitions.join(", "));
    sql.push(')');
    sql.push(';');
    Ok(sql)
}

fn source_table_sql_hint(table: &stateql_core::Table) -> Option<&str> {
    match table.options.extra.get(extra_keys::TABLE_SOURCE_SQL) {
        Some(Value::String(value)) if !value.trim().is_empty() => Some(value.as_str()),
        _ => None,
    }
}

fn render_column(
    dialect_name: &str,
    column: &stateql_core::Column,
) -> stateql_core::Result<String> {
    if column.generated.is_some() {
        return unsupported_shape_error(dialect_name, UNSUPPORTED_TABLE_VARIANT);
    }

    let mut sql = format!(
        "{} {}",
        render_ident(&column.name),
        render_data_type(&column.data_type)
    );

    if let Some(default) = &column.default {
        if let Some(Value::String(name)) = column
            .extra
            .get(stateql_core::extra_keys::mssql::DEFAULT_CONSTRAINT_NAME)
        {
            write!(
                sql,
                " CONSTRAINT {} DEFAULT {}",
                render_ident(&Ident::unquoted(name)),
                render_expr(default)
            )
            .expect("writing to String should not fail");
        } else {
            write!(sql, " DEFAULT {}", render_expr(default))
                .expect("writing to String should not fail");
        }
    }

    if let Some(identity) = &column.identity {
        let seed = identity.start.unwrap_or(1);
        let increment = identity.increment.unwrap_or(1);
        write!(sql, " IDENTITY({seed},{increment})").expect("writing to String should not fail");

        if column
            .extra
            .get(extra_keys::COLUMN_IDENTITY_NOT_FOR_REPLICATION)
            .is_some_and(|value| matches!(value, Value::Bool(true)))
        {
            sql.push_str(" NOT FOR REPLICATION");
        }
    }

    if column.not_null {
        sql.push_str(" NOT NULL");
    }
    if let Some(collation) = &column.collation {
        write!(sql, " COLLATE {}", collation.trim()).expect("writing to String should not fail");
    }

    Ok(sql)
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
    if view.query.trim().is_empty() {
        return unsupported_variant_error("mssql", "CreateViewUnsupportedShape");
    }

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
    let Some(name) = &index.name else {
        return unsupported_shape_error(dialect_name, UNSUPPORTED_INDEX_VARIANT);
    };

    if index.columns.is_empty() {
        return unsupported_shape_error(dialect_name, UNSUPPORTED_INDEX_VARIANT);
    }

    let owner = match &index.owner {
        IndexOwner::Table(owner)
        | IndexOwner::View(owner)
        | IndexOwner::MaterializedView(owner) => owner,
    };

    let mut sql = String::from("CREATE ");
    if index.unique {
        sql.push_str("UNIQUE ");
    }

    if let Some(method) = &index.method {
        let upper = method.trim().to_ascii_uppercase();
        if upper == "CLUSTERED" || upper == "NONCLUSTERED" {
            write!(sql, "{upper} ").expect("writing to String should not fail");
        }
    }

    write!(
        sql,
        "INDEX {} ON {} ({})",
        render_ident(name),
        render_qualified_name(owner),
        index
            .columns
            .iter()
            .map(|column| render_expr(&column.expr))
            .collect::<Vec<_>>()
            .join(", "),
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
    if trigger.events.is_empty() || trigger.body.trim().is_empty() {
        return unsupported_shape_error(dialect_name, UNSUPPORTED_TRIGGER_VARIANT);
    }

    let mut sql = format!(
        "CREATE TRIGGER {} ON {} {} {} AS ",
        render_qualified_name(&trigger.name),
        render_qualified_name(&trigger.table),
        render_trigger_timing(trigger.timing),
        trigger
            .events
            .iter()
            .map(|event| render_trigger_event(*event))
            .collect::<Vec<_>>()
            .join(", "),
    );

    let body = trigger.body.trim().trim_end_matches(';').trim();
    if body.to_ascii_uppercase().starts_with("BEGIN") {
        sql.push_str(body);
    } else {
        write!(sql, "BEGIN {body}; END").expect("writing to String should not fail");
    }
    sql.push(';');

    Ok(sql)
}

fn render_function(dialect_name: &str, function: &Function) -> stateql_core::Result<String> {
    let Some(return_type) = &function.return_type else {
        return unsupported_shape_error(dialect_name, UNSUPPORTED_FUNCTION_VARIANT);
    };

    let body = function.body.trim().trim_end_matches(';').trim();
    if body.is_empty() {
        return unsupported_shape_error(dialect_name, UNSUPPORTED_FUNCTION_VARIANT);
    }

    let params = function
        .params
        .iter()
        .map(render_function_param)
        .collect::<Vec<_>>()
        .join(", ");

    let mut sql = format!(
        "CREATE FUNCTION {}({params}) RETURNS {} AS ",
        render_qualified_name(&function.name),
        render_data_type(return_type)
    );

    if body.to_ascii_uppercase().starts_with("BEGIN") {
        sql.push_str(body);
    } else {
        write!(sql, "BEGIN {body}; END").expect("writing to String should not fail");
    }
    sql.push(';');

    Ok(sql)
}

fn render_function_param(param: &stateql_core::FunctionParam) -> String {
    let mut sql = String::new();

    if let Some(name) = &param.name {
        let mut parameter_name = name.value.clone();
        if !parameter_name.starts_with('@') {
            parameter_name.insert(0, '@');
        }
        write!(sql, "{parameter_name} ").expect("writing to String should not fail");
    }

    write!(sql, "{}", render_data_type(&param.data_type))
        .expect("writing to String should not fail");

    if let Some(default) = &param.default {
        write!(sql, " = {}", render_expr(default)).expect("writing to String should not fail");
    }

    match param.mode {
        Some(FunctionParamMode::Out) | Some(FunctionParamMode::InOut) => sql.push_str(" OUTPUT"),
        Some(FunctionParamMode::In) | Some(FunctionParamMode::Variadic) | None => {}
    }

    sql
}

fn render_schema(schema: &SchemaDef) -> stateql_core::Result<String> {
    Ok(format!("CREATE SCHEMA {};", render_ident(&schema.name)))
}

fn render_data_type(data_type: &DataType) -> String {
    match data_type {
        DataType::Boolean => "BIT".to_string(),
        DataType::SmallInt => "SMALLINT".to_string(),
        DataType::Integer => "INT".to_string(),
        DataType::BigInt => "BIGINT".to_string(),
        DataType::Real => "REAL".to_string(),
        DataType::DoublePrecision => "FLOAT".to_string(),
        DataType::Numeric { precision, scale } => match (precision, scale) {
            (Some(precision), Some(scale)) => format!("DECIMAL({precision}, {scale})"),
            (Some(precision), None) => format!("DECIMAL({precision})"),
            _ => "DECIMAL".to_string(),
        },
        DataType::Text => "NVARCHAR(MAX)".to_string(),
        DataType::Varchar { length } => match length {
            Some(length) => format!("NVARCHAR({length})"),
            None => "NVARCHAR(MAX)".to_string(),
        },
        DataType::Char { length } => match length {
            Some(length) => format!("NCHAR({length})"),
            None => "NCHAR(1)".to_string(),
        },
        DataType::Blob => "VARBINARY(MAX)".to_string(),
        DataType::Date => "DATE".to_string(),
        DataType::Time { .. } => "TIME".to_string(),
        DataType::Timestamp { .. } => "DATETIME2".to_string(),
        DataType::Json | DataType::Jsonb => "NVARCHAR(MAX)".to_string(),
        DataType::Uuid => "UNIQUEIDENTIFIER".to_string(),
        DataType::Array(_) => "NVARCHAR(MAX)".to_string(),
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
                render_binary_operator(op),
                render_expr(right)
            )
        }
        Expr::UnaryOp { op, expr } => {
            let operand = render_expr(expr);
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
                render_expr(left),
                render_comparison_op(op),
                render_expr(right)
            );
            if let Some(quantifier) = quantifier {
                write!(sql, " {}", render_quantifier(quantifier))
                    .expect("writing to String should not fail");
            }
            sql
        }
        Expr::And(left, right) => format!("{} AND {}", render_expr(left), render_expr(right)),
        Expr::Or(left, right) => format!("{} OR {}", render_expr(left), render_expr(right)),
        Expr::Not(inner) => format!("NOT {}", render_expr(inner)),
        Expr::Is { expr, test } => format!("{} IS {}", render_expr(expr), render_is_test(test)),
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
                let mut clauses = Vec::new();
                if !window_spec.partition_by.is_empty() {
                    clauses.push(format!(
                        "PARTITION BY {}",
                        window_spec
                            .partition_by
                            .iter()
                            .map(render_expr)
                            .collect::<Vec<_>>()
                            .join(", ")
                    ));
                }
                if !window_spec.order_by.is_empty() {
                    clauses.push(format!(
                        "ORDER BY {}",
                        window_spec
                            .order_by
                            .iter()
                            .map(render_expr)
                            .collect::<Vec<_>>()
                            .join(", ")
                    ));
                }
                sql.push_str(&clauses.join(" "));
                sql.push(')');
            }
            sql
        }
        Expr::Cast { expr, data_type } => {
            format!(
                "CAST({} AS {})",
                render_expr(expr),
                render_data_type(data_type)
            )
        }
        Expr::Collate { expr, collation } => {
            format!("{} COLLATE {}", render_expr(expr), collation.trim())
        }
        Expr::Case {
            operand,
            when_clauses,
            else_clause,
        } => {
            let mut sql = String::from("CASE");
            if let Some(operand) = operand {
                write!(sql, " {}", render_expr(operand))
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
                write!(sql, " ELSE {}", render_expr(else_expr))
                    .expect("writing to String should not fail");
            }
            sql.push_str(" END");
            sql
        }
        Expr::ArrayConstructor(items) => format!(
            "({})",
            items.iter().map(render_expr).collect::<Vec<_>>().join(", ")
        ),
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

fn render_binary_operator(op: &BinaryOperator) -> &'static str {
    match op {
        BinaryOperator::Add => "+",
        BinaryOperator::Subtract => "-",
        BinaryOperator::Multiply => "*",
        BinaryOperator::Divide => "/",
        BinaryOperator::Modulo => "%",
        BinaryOperator::StringConcat => "+",
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

fn render_quantifier(quantifier: &stateql_core::SetQuantifier) -> &'static str {
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
        TriggerTiming::Before => "AFTER",
        TriggerTiming::After => "AFTER",
        TriggerTiming::InsteadOf => "INSTEAD OF",
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

fn render_qualified_name(name: &stateql_core::QualifiedName) -> String {
    if let Some(schema) = &name.schema {
        format!("{}.{}", render_ident(schema), render_ident(&name.name))
    } else {
        render_ident(&name.name)
    }
}

fn render_ident(ident: &Ident) -> String {
    format!("[{}]", ident.value.replace(']', "]]"))
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
