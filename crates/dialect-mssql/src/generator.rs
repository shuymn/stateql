use std::fmt::Write as _;

use stateql_core::{
    BinaryOperator, CheckConstraint, Column, ComparisonOp, DataType, DiffOp, Expr, ForeignKey,
    ForeignKeyAction, GenerateError, Ident, IndexOwner, IsTest, Literal, PrimaryKey, QualifiedName,
    Result, SchemaObject, SetQuantifier, Statement, UnaryOperator, Value,
};

use crate::{extra_keys, to_sql};

const GENERATOR_TARGET: &str = "mssql ddl generator";

pub(crate) fn generate_ddl(dialect_name: &str, ops: &[DiffOp]) -> Result<Vec<Statement>> {
    let mut statements = Vec::new();

    for op in ops {
        emit_op(dialect_name, op, &mut statements)?;
    }

    Ok(statements)
}

fn emit_op(dialect_name: &str, op: &DiffOp, out: &mut Vec<Statement>) -> Result<()> {
    match op {
        DiffOp::CreateTable(table) => {
            let sql = render_object_sql(dialect_name, op, &SchemaObject::Table(table.clone()))?;
            append_sql(out, sql);
        }
        DiffOp::DropTable(name) => {
            append_sql(out, format!("DROP TABLE {};", render_qualified_name(name)));
        }
        DiffOp::RenameTable { from, to } => {
            if from.schema != to.schema {
                return Err(unsupported_diff_op(
                    dialect_name,
                    op,
                    "mssql cannot rename tables across schemas",
                ));
            }
            append_sql(out, render_table_rename(from, to));
        }
        DiffOp::AddColumn {
            table,
            column,
            position,
        } => {
            if position.is_some() {
                return Err(unsupported_diff_op(
                    dialect_name,
                    op,
                    "mssql add-column does not support position",
                ));
            }
            let definition = render_column_definition(dialect_name, op, column)?;
            append_sql(
                out,
                format!(
                    "ALTER TABLE {} ADD {definition};",
                    render_qualified_name(table)
                ),
            );
        }
        DiffOp::DropColumn { table, column } => {
            append_sql(
                out,
                format!(
                    "ALTER TABLE {} DROP COLUMN {};",
                    render_qualified_name(table),
                    render_ident(column)
                ),
            );
        }
        DiffOp::AlterColumn { .. } => {
            return Err(unsupported_diff_op(dialect_name, op, GENERATOR_TARGET));
        }
        DiffOp::RenameColumn { table, from, to } => {
            append_sql(out, render_column_rename(table, from, to));
        }
        DiffOp::AddIndex(index) => {
            if !matches!(index.owner, IndexOwner::Table(_)) {
                return Err(unsupported_diff_op(
                    dialect_name,
                    op,
                    "mssql indexes must target tables",
                ));
            }
            let sql = render_object_sql(dialect_name, op, &SchemaObject::Index(index.clone()))?;
            append_sql(out, sql);
        }
        DiffOp::DropIndex { owner, name } => {
            let owner_table = match owner {
                IndexOwner::Table(table) => table,
                IndexOwner::View(_) | IndexOwner::MaterializedView(_) => {
                    return Err(unsupported_diff_op(
                        dialect_name,
                        op,
                        "mssql indexes must target tables",
                    ));
                }
            };
            append_sql(
                out,
                format!(
                    "DROP INDEX {} ON {};",
                    render_ident(name),
                    render_qualified_name(owner_table)
                ),
            );
        }
        DiffOp::RenameIndex { owner, from, to } => {
            let owner_table = match owner {
                IndexOwner::Table(table) => table,
                IndexOwner::View(_) | IndexOwner::MaterializedView(_) => {
                    return Err(unsupported_diff_op(
                        dialect_name,
                        op,
                        "mssql indexes must target tables",
                    ));
                }
            };
            append_sql(out, render_index_rename(owner_table, from, to));
        }
        DiffOp::AddForeignKey { table, fk } => {
            if fk.deferrable.is_some() {
                return Err(unsupported_diff_op(
                    dialect_name,
                    op,
                    "mssql does not support deferrable foreign keys",
                ));
            }
            let definition = render_foreign_key(fk);
            append_sql(
                out,
                format!(
                    "ALTER TABLE {} ADD {definition};",
                    render_qualified_name(table)
                ),
            );
        }
        DiffOp::DropForeignKey { table, name } => {
            append_sql(
                out,
                format!(
                    "ALTER TABLE {} DROP CONSTRAINT {};",
                    render_qualified_name(table),
                    render_ident(name)
                ),
            );
        }
        DiffOp::AddCheck { table, check } => {
            if check.no_inherit {
                return Err(unsupported_diff_op(
                    dialect_name,
                    op,
                    "mssql does not support NO INHERIT checks",
                ));
            }
            let definition = render_check(check);
            append_sql(
                out,
                format!(
                    "ALTER TABLE {} ADD {definition};",
                    render_qualified_name(table)
                ),
            );
        }
        DiffOp::DropCheck { table, name } => {
            append_sql(
                out,
                format!(
                    "ALTER TABLE {} DROP CONSTRAINT {};",
                    render_qualified_name(table),
                    render_ident(name)
                ),
            );
        }
        DiffOp::SetPrimaryKey { table, pk } => {
            let definition = render_primary_key(pk);
            append_sql(
                out,
                format!(
                    "ALTER TABLE {} ADD {definition};",
                    render_qualified_name(table)
                ),
            );
        }
        DiffOp::DropPrimaryKey { .. } => {
            return Err(unsupported_diff_op(
                dialect_name,
                op,
                "mssql drop primary key requires a named constraint",
            ));
        }
        DiffOp::CreateView(view) => {
            let sql = render_object_sql(dialect_name, op, &SchemaObject::View(view.clone()))?;
            append_sql(out, sql);
        }
        DiffOp::DropView(name) => {
            append_sql(out, format!("DROP VIEW {};", render_qualified_name(name)));
        }
        DiffOp::CreateTrigger(trigger) => {
            let sql = render_object_sql(dialect_name, op, &SchemaObject::Trigger(trigger.clone()))?;
            append_sql(out, sql);
        }
        DiffOp::DropTrigger { name, .. } => {
            append_sql(
                out,
                format!("DROP TRIGGER {};", render_qualified_name(name)),
            );
        }
        DiffOp::CreateFunction(function) => {
            let sql =
                render_object_sql(dialect_name, op, &SchemaObject::Function(function.clone()))?;
            append_sql(out, sql);
        }
        DiffOp::DropFunction(name) => {
            append_sql(
                out,
                format!("DROP FUNCTION {};", render_qualified_name(name)),
            );
        }
        DiffOp::CreateSchema(schema) => {
            let sql = render_object_sql(dialect_name, op, &SchemaObject::Schema(schema.clone()))?;
            append_sql(out, sql);
        }
        DiffOp::DropSchema(name) => {
            append_sql(out, format!("DROP SCHEMA {};", render_qualified_name(name)));
        }
        _ => return Err(unsupported_diff_op(dialect_name, op, GENERATOR_TARGET)),
    }

    Ok(())
}

fn render_table_rename(from: &QualifiedName, to: &QualifiedName) -> String {
    render_sp_rename(&sp_rename_table_target(from), &to.name.value, None)
}

fn render_column_rename(table: &QualifiedName, from: &Ident, to: &Ident) -> String {
    let target = format!("{}.{}", sp_rename_table_target(table), from.value);
    render_sp_rename(&target, &to.value, Some("COLUMN"))
}

fn render_index_rename(table: &QualifiedName, from: &Ident, to: &Ident) -> String {
    let target = format!("{}.{}", sp_rename_table_target(table), from.value);
    render_sp_rename(&target, &to.value, Some("INDEX"))
}

fn render_sp_rename(target: &str, new_name: &str, kind: Option<&str>) -> String {
    let escaped_target = escape_sql_literal(target);
    let escaped_new_name = escape_sql_literal(new_name);

    if let Some(kind) = kind {
        format!("EXEC sp_rename '{escaped_target}', '{escaped_new_name}', '{kind}';")
    } else {
        format!("EXEC sp_rename '{escaped_target}', '{escaped_new_name}';")
    }
}

fn sp_rename_table_target(table: &QualifiedName) -> String {
    if let Some(schema) = &table.schema {
        format!("{}.{}", schema.value, table.name.value)
    } else {
        table.name.value.clone()
    }
}

fn escape_sql_literal(value: &str) -> String {
    value.replace('\'', "''")
}

fn render_column_definition(dialect_name: &str, op: &DiffOp, column: &Column) -> Result<String> {
    if column.generated.is_some() {
        return Err(unsupported_diff_op(
            dialect_name,
            op,
            "mssql add-column does not support generated columns",
        ));
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
            .join(", "),
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

fn render_primary_key(pk: &PrimaryKey) -> String {
    let mut sql = String::new();

    if let Some(name) = &pk.name {
        write!(sql, "CONSTRAINT {} ", render_ident(name))
            .expect("writing to String should not fail");
    }

    write!(
        sql,
        "PRIMARY KEY ({})",
        pk.columns
            .iter()
            .map(render_ident)
            .collect::<Vec<_>>()
            .join(", "),
    )
    .expect("writing to String should not fail");

    sql
}

fn append_sql(out: &mut Vec<Statement>, sql: String) {
    if matches!(out.last(), Some(Statement::Sql { .. })) {
        out.push(Statement::BatchBoundary);
    }

    out.push(sql_statement(sql, true));
}

fn sql_statement(sql: String, transactional: bool) -> Statement {
    Statement::Sql {
        sql: ensure_sql_terminated(&sql),
        transactional,
        context: None,
    }
}

fn ensure_sql_terminated(sql: &str) -> String {
    let trimmed = sql.trim();
    if trimmed.ends_with(';') {
        trimmed.to_string()
    } else {
        format!("{trimmed};")
    }
}

fn render_object_sql(dialect_name: &str, op: &DiffOp, object: &SchemaObject) -> Result<String> {
    match to_sql::render_object(dialect_name, object) {
        Ok(sql) => Ok(sql),
        Err(stateql_core::Error::Generate(GenerateError::UnsupportedDiffOp { target, .. })) => {
            Err(unsupported_diff_op(dialect_name, op, target))
        }
        Err(other) => Err(other),
    }
}

fn unsupported_diff_op(
    dialect_name: &str,
    op: &DiffOp,
    target: impl Into<String>,
) -> stateql_core::Error {
    GenerateError::UnsupportedDiffOp {
        diff_op: diff_op_tag(op).to_string(),
        target: target.into(),
        dialect: dialect_name.to_string(),
    }
    .into()
}

fn diff_op_tag(op: &DiffOp) -> &'static str {
    match op {
        DiffOp::CreateTable(_) => "CreateTable",
        DiffOp::DropTable(_) => "DropTable",
        DiffOp::RenameTable { .. } => "RenameTable",
        DiffOp::AddColumn { .. } => "AddColumn",
        DiffOp::DropColumn { .. } => "DropColumn",
        DiffOp::AlterColumn { .. } => "AlterColumn",
        DiffOp::RenameColumn { .. } => "RenameColumn",
        DiffOp::AddIndex(_) => "AddIndex",
        DiffOp::DropIndex { .. } => "DropIndex",
        DiffOp::RenameIndex { .. } => "RenameIndex",
        DiffOp::AddForeignKey { .. } => "AddForeignKey",
        DiffOp::DropForeignKey { .. } => "DropForeignKey",
        DiffOp::AddCheck { .. } => "AddCheck",
        DiffOp::DropCheck { .. } => "DropCheck",
        DiffOp::AddExclusion { .. } => "AddExclusion",
        DiffOp::DropExclusion { .. } => "DropExclusion",
        DiffOp::SetPrimaryKey { .. } => "SetPrimaryKey",
        DiffOp::DropPrimaryKey { .. } => "DropPrimaryKey",
        DiffOp::AddPartition { .. } => "AddPartition",
        DiffOp::DropPartition { .. } => "DropPartition",
        DiffOp::CreateView(_) => "CreateView",
        DiffOp::DropView(_) => "DropView",
        DiffOp::CreateMaterializedView(_) => "CreateMaterializedView",
        DiffOp::DropMaterializedView(_) => "DropMaterializedView",
        DiffOp::CreateSequence(_) => "CreateSequence",
        DiffOp::DropSequence(_) => "DropSequence",
        DiffOp::AlterSequence { .. } => "AlterSequence",
        DiffOp::CreateTrigger(_) => "CreateTrigger",
        DiffOp::DropTrigger { .. } => "DropTrigger",
        DiffOp::CreateFunction(_) => "CreateFunction",
        DiffOp::DropFunction(_) => "DropFunction",
        DiffOp::CreateType(_) => "CreateType",
        DiffOp::DropType(_) => "DropType",
        DiffOp::AlterType { .. } => "AlterType",
        DiffOp::CreateDomain(_) => "CreateDomain",
        DiffOp::DropDomain(_) => "DropDomain",
        DiffOp::AlterDomain { .. } => "AlterDomain",
        DiffOp::CreateExtension(_) => "CreateExtension",
        DiffOp::DropExtension(_) => "DropExtension",
        DiffOp::CreateSchema(_) => "CreateSchema",
        DiffOp::DropSchema(_) => "DropSchema",
        DiffOp::SetComment(_) => "SetComment",
        DiffOp::DropComment { .. } => "DropComment",
        DiffOp::Grant(_) => "Grant",
        DiffOp::Revoke(_) => "Revoke",
        DiffOp::CreatePolicy(_) => "CreatePolicy",
        DiffOp::DropPolicy { .. } => "DropPolicy",
        DiffOp::AlterTableOptions { .. } => "AlterTableOptions",
    }
}

fn render_qualified_name(name: &QualifiedName) -> String {
    if let Some(schema) = &name.schema {
        format!("{}.{}", render_ident(schema), render_ident(&name.name))
    } else {
        render_ident(&name.name)
    }
}

fn render_ident(ident: &Ident) -> String {
    format!("[{}]", ident.value.replace(']', "]]"))
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

fn render_quantifier(quantifier: &SetQuantifier) -> &'static str {
    match quantifier {
        SetQuantifier::Any => "ANY",
        SetQuantifier::Some => "SOME",
        SetQuantifier::All => "ALL",
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
