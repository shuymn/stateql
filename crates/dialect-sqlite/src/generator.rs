use std::fmt::Write as _;

use stateql_core::{
    Column, ColumnChange, DataType, DiffOp, Error, Expr, GenerateError, Ident, IndexOwner, Literal,
    QualifiedName, Result, SchemaObject, SqliteRebuildStep, Statement, StatementContext, Value,
};

use crate::to_sql;

const GENERATOR_TARGET: &str = "sqlite ddl generator";
const SHADOW_TABLE_PREFIX: &str = "__stateql_rebuild_";

pub(crate) fn generate_ddl(dialect_name: &str, ops: &[DiffOp]) -> Result<Vec<Statement>> {
    let mut statements = Vec::new();
    let mut index = 0usize;

    while index < ops.len() {
        if let Some(table) = rebuild_table(&ops[index]).cloned() {
            let start = index;
            index += 1;

            while let Some(next_op) = ops.get(index) {
                if rebuild_table(next_op) == Some(&table) {
                    index += 1;
                } else {
                    break;
                }
            }

            statements.extend(build_sqlite_rebuild_plan(
                dialect_name,
                &table,
                &ops[start..index],
            )?);
            continue;
        }

        emit_simple_op(dialect_name, &ops[index], &mut statements)?;
        index += 1;
    }

    Ok(statements)
}

fn emit_simple_op(dialect_name: &str, op: &DiffOp, out: &mut Vec<Statement>) -> Result<()> {
    match op {
        DiffOp::CreateTable(table) => {
            let sql = render_schema_object(dialect_name, op, SchemaObject::Table(table.clone()))?;
            out.push(sql_statement(sql, true, None));
        }
        DiffOp::DropTable(name) => {
            out.push(sql_statement(
                format!("DROP TABLE {};", render_qualified_name(name)),
                true,
                None,
            ));
        }
        DiffOp::RenameTable { from, to } => {
            if from.schema != to.schema {
                return Err(unsupported_diff_op(
                    dialect_name,
                    op,
                    "sqlite cannot rename across schemas",
                ));
            }
            out.push(sql_statement(
                format!(
                    "ALTER TABLE {} RENAME TO {};",
                    render_qualified_name(from),
                    render_ident(&to.name)
                ),
                true,
                None,
            ));
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
                    "sqlite add-column supports append only",
                ));
            }
            let column_definition = render_column_definition(column, dialect_name, op)?;
            out.push(sql_statement(
                format!(
                    "ALTER TABLE {} ADD COLUMN {column_definition};",
                    render_qualified_name(table)
                ),
                true,
                None,
            ));
        }
        DiffOp::RenameColumn { table, from, to } => {
            out.push(sql_statement(
                format!(
                    "ALTER TABLE {} RENAME COLUMN {} TO {};",
                    render_qualified_name(table),
                    render_ident(from),
                    render_ident(to)
                ),
                true,
                None,
            ));
        }
        DiffOp::AddIndex(index) => {
            if !matches!(index.owner, IndexOwner::Table(_)) {
                return Err(unsupported_diff_op(
                    dialect_name,
                    op,
                    "sqlite indexes are table-scoped",
                ));
            }
            let sql = render_schema_object(dialect_name, op, SchemaObject::Index(index.clone()))?;
            out.push(sql_statement(sql, true, None));
        }
        DiffOp::DropIndex { owner, name } => {
            if !matches!(owner, IndexOwner::Table(_)) {
                return Err(unsupported_diff_op(
                    dialect_name,
                    op,
                    "sqlite indexes are table-scoped",
                ));
            }
            out.push(sql_statement(
                format!("DROP INDEX {};", render_ident(name)),
                true,
                None,
            ));
        }
        DiffOp::CreateView(view) => {
            let sql = render_schema_object(dialect_name, op, SchemaObject::View(view.clone()))?;
            out.push(sql_statement(sql, true, None));
        }
        DiffOp::DropView(name) => {
            out.push(sql_statement(
                format!("DROP VIEW {};", render_qualified_name(name)),
                true,
                None,
            ));
        }
        DiffOp::CreateTrigger(trigger) => {
            let sql =
                render_schema_object(dialect_name, op, SchemaObject::Trigger(trigger.clone()))?;
            out.push(sql_statement(sql, true, None));
        }
        DiffOp::DropTrigger { name, .. } => {
            out.push(sql_statement(
                format!("DROP TRIGGER {};", render_qualified_name(name)),
                true,
                None,
            ));
        }
        DiffOp::AlterColumn { .. }
        | DiffOp::DropColumn { .. }
        | DiffOp::AddForeignKey { .. }
        | DiffOp::DropForeignKey { .. }
        | DiffOp::AddCheck { .. }
        | DiffOp::DropCheck { .. }
        | DiffOp::AddExclusion { .. }
        | DiffOp::DropExclusion { .. }
        | DiffOp::SetPrimaryKey { .. }
        | DiffOp::DropPrimaryKey { .. } => {
            return Err(unsupported_diff_op(
                dialect_name,
                op,
                "sqlite rebuild ops must be batched before emit_simple_op",
            ));
        }
        _ => {
            return Err(unsupported_diff_op(dialect_name, op, GENERATOR_TARGET));
        }
    }

    Ok(())
}

fn build_sqlite_rebuild_plan(
    dialect_name: &str,
    table: &QualifiedName,
    ops: &[DiffOp],
) -> Result<Vec<Statement>> {
    let first_op = ops
        .first()
        .ok_or_else(|| GenerateError::UnsupportedDiffOp {
            diff_op: "SqliteRebuild".to_string(),
            target: GENERATOR_TARGET.to_string(),
            dialect: dialect_name.to_string(),
        })?;

    let rebuilt_columns = collect_rebuild_columns(dialect_name, table, ops)?;
    let shadow_table = shadow_table_name(table);
    let create_sql = if rebuilt_columns.is_empty() {
        format!(
            "CREATE TABLE {} AS SELECT * FROM {} WHERE 0;",
            render_qualified_name(&shadow_table),
            render_qualified_name(table)
        )
    } else {
        let column_sql = rebuilt_columns
            .iter()
            .map(|column| render_rebuild_column(column, dialect_name, first_op))
            .collect::<Result<Vec<_>>>()?;
        format!(
            "CREATE TABLE {} ({});",
            render_qualified_name(&shadow_table),
            column_sql.join(", ")
        )
    };

    let copy_sql = if rebuilt_columns.is_empty() {
        format!(
            "INSERT INTO {} SELECT * FROM {};",
            render_qualified_name(&shadow_table),
            render_qualified_name(table)
        )
    } else {
        let column_list = rebuilt_columns
            .iter()
            .map(|column| render_ident(&column.name))
            .collect::<Vec<_>>()
            .join(", ");
        let select_list = rebuilt_columns
            .iter()
            .map(render_copy_projection)
            .collect::<Vec<_>>()
            .join(", ");
        format!(
            "INSERT INTO {} ({column_list}) SELECT {select_list} FROM {};",
            render_qualified_name(&shadow_table),
            render_qualified_name(table)
        )
    };

    Ok(vec![
        rebuild_statement(create_sql, table, SqliteRebuildStep::CreateShadowTable),
        rebuild_statement(copy_sql, table, SqliteRebuildStep::CopyData),
        rebuild_statement(
            format!("DROP TABLE {};", render_qualified_name(table)),
            table,
            SqliteRebuildStep::DropOldTable,
        ),
        rebuild_statement(
            format!(
                "ALTER TABLE {} RENAME TO {};",
                render_qualified_name(&shadow_table),
                render_ident(&table.name)
            ),
            table,
            SqliteRebuildStep::RenameShadowTable,
        ),
        rebuild_statement(
            format!(
                "-- sqlite rebuild: recreate indexes for {}",
                render_qualified_name(table)
            ),
            table,
            SqliteRebuildStep::RecreateIndexes,
        ),
        rebuild_statement(
            format!(
                "-- sqlite rebuild: recreate triggers for {}",
                render_qualified_name(table)
            ),
            table,
            SqliteRebuildStep::RecreateTriggers,
        ),
    ])
}

fn collect_rebuild_columns(
    dialect_name: &str,
    table: &QualifiedName,
    ops: &[DiffOp],
) -> Result<Vec<RebuildColumnSpec>> {
    let mut columns = Vec::new();

    for op in ops {
        match op {
            DiffOp::AlterColumn {
                table: target_table,
                column,
                changes,
            } => {
                ensure_same_table(dialect_name, op, table, target_table)?;

                let spec = upsert_rebuild_column(&mut columns, column.clone());
                for change in changes {
                    match change {
                        ColumnChange::SetType(data_type) => {
                            spec.data_type = Some(data_type.clone());
                        }
                        ColumnChange::SetNotNull(not_null) => {
                            spec.not_null = Some(*not_null);
                        }
                        ColumnChange::SetDefault(default) => {
                            spec.default = Some(default.clone());
                        }
                        ColumnChange::SetIdentity(_)
                        | ColumnChange::SetGenerated(_)
                        | ColumnChange::SetCollation(_) => {}
                    }
                }
            }
            DiffOp::DropColumn {
                table: target_table,
                ..
            }
            | DiffOp::AddForeignKey {
                table: target_table,
                ..
            }
            | DiffOp::DropForeignKey {
                table: target_table,
                ..
            }
            | DiffOp::AddCheck {
                table: target_table,
                ..
            }
            | DiffOp::DropCheck {
                table: target_table,
                ..
            }
            | DiffOp::AddExclusion {
                table: target_table,
                ..
            }
            | DiffOp::DropExclusion {
                table: target_table,
                ..
            }
            | DiffOp::SetPrimaryKey {
                table: target_table,
                ..
            }
            | DiffOp::DropPrimaryKey {
                table: target_table,
                ..
            } => {
                ensure_same_table(dialect_name, op, table, target_table)?;
            }
            _ => {
                return Err(unsupported_diff_op(
                    dialect_name,
                    op,
                    "unexpected op in sqlite rebuild batch",
                ));
            }
        }
    }

    Ok(columns)
}

fn ensure_same_table(
    dialect_name: &str,
    op: &DiffOp,
    expected: &QualifiedName,
    actual: &QualifiedName,
) -> Result<()> {
    if expected == actual {
        return Ok(());
    }
    Err(unsupported_diff_op(
        dialect_name,
        op,
        "sqlite rebuild batch mixed multiple tables",
    ))
}

fn render_rebuild_column(
    column: &RebuildColumnSpec,
    dialect_name: &str,
    op: &DiffOp,
) -> Result<String> {
    let mut rendered = format!(
        "{} {}",
        render_ident(&column.name),
        render_data_type(column.data_type.as_ref().unwrap_or(&DataType::Text))
    );

    if column.not_null.unwrap_or(false) {
        rendered.push_str(" NOT NULL");
    }

    if let Some(default) = &column.default
        && let Some(default_expr) = default
    {
        write!(
            rendered,
            " DEFAULT {}",
            render_default_expr(default_expr, dialect_name, op)?
        )
        .expect("writing to String should not fail");
    }

    Ok(rendered)
}

fn render_copy_projection(column: &RebuildColumnSpec) -> String {
    let source_column = render_ident(&column.name);
    if let Some(data_type) = &column.data_type {
        format!("CAST({source_column} AS {})", render_data_type(data_type))
    } else {
        source_column
    }
}

fn render_column_definition(column: &Column, dialect_name: &str, op: &DiffOp) -> Result<String> {
    let mut sql = format!(
        "{} {}",
        render_ident(&column.name),
        render_data_type(&column.data_type)
    );

    if column.not_null {
        sql.push_str(" NOT NULL");
    }

    if let Some(default) = &column.default {
        write!(
            sql,
            " DEFAULT {}",
            render_default_expr(default, dialect_name, op)?
        )
        .expect("writing to String should not fail");
    }

    Ok(sql)
}

fn render_default_expr(expr: &Expr, dialect_name: &str, op: &DiffOp) -> Result<String> {
    match expr {
        Expr::Literal(literal) => Ok(render_literal(literal)),
        Expr::Raw(raw) => Ok(raw.trim().to_string()),
        Expr::Null => Ok("NULL".to_string()),
        Expr::Ident(ident) => Ok(render_ident(ident)),
        Expr::QualifiedIdent { qualifier, name } => Ok(format!(
            "{}.{}",
            render_ident(qualifier),
            render_ident(name)
        )),
        Expr::Cast { expr, data_type } => Ok(format!(
            "CAST({} AS {})",
            render_default_expr(expr, dialect_name, op)?,
            render_data_type(data_type)
        )),
        Expr::Paren(inner) => Ok(format!(
            "({})",
            render_default_expr(inner, dialect_name, op)?
        )),
        _ => Err(unsupported_diff_op(
            dialect_name,
            op,
            "unsupported expression in sqlite default value",
        )),
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

fn render_schema_object(dialect_name: &str, op: &DiffOp, object: SchemaObject) -> Result<String> {
    to_sql::render_object(dialect_name, &object).map_err(|error| match error {
        Error::Generate(GenerateError::UnsupportedDiffOp { target, .. }) => {
            unsupported_diff_op(dialect_name, op, target)
        }
        other => other,
    })
}

fn rebuild_table(op: &DiffOp) -> Option<&QualifiedName> {
    match op {
        DiffOp::AlterColumn { table, .. }
        | DiffOp::DropColumn { table, .. }
        | DiffOp::AddForeignKey { table, .. }
        | DiffOp::DropForeignKey { table, .. }
        | DiffOp::AddCheck { table, .. }
        | DiffOp::DropCheck { table, .. }
        | DiffOp::AddExclusion { table, .. }
        | DiffOp::DropExclusion { table, .. }
        | DiffOp::SetPrimaryKey { table, .. }
        | DiffOp::DropPrimaryKey { table } => Some(table),
        _ => None,
    }
}

fn shadow_table_name(table: &QualifiedName) -> QualifiedName {
    QualifiedName {
        schema: table.schema.clone(),
        name: Ident::unquoted(format!("{SHADOW_TABLE_PREFIX}{}", table.name.value)),
    }
}

fn rebuild_statement(sql: String, table: &QualifiedName, step: SqliteRebuildStep) -> Statement {
    sql_statement(
        sql,
        true,
        Some(StatementContext::SqliteTableRebuild {
            table: table.clone(),
            step,
        }),
    )
}

fn sql_statement(sql: String, transactional: bool, context: Option<StatementContext>) -> Statement {
    Statement::Sql {
        sql,
        transactional,
        context,
    }
}

fn render_qualified_name(name: &QualifiedName) -> String {
    match &name.schema {
        Some(schema) => format!("{}.{}", render_ident(schema), render_ident(&name.name)),
        None => render_ident(&name.name),
    }
}

fn render_ident(ident: &Ident) -> String {
    format!("\"{}\"", ident.value.replace('"', "\"\""))
}

fn unsupported_diff_op(dialect_name: &str, op: &DiffOp, target: impl Into<String>) -> Error {
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

#[derive(Debug, Clone)]
struct RebuildColumnSpec {
    name: Ident,
    data_type: Option<DataType>,
    not_null: Option<bool>,
    default: Option<Option<Expr>>,
}

fn upsert_rebuild_column(
    columns: &mut Vec<RebuildColumnSpec>,
    name: Ident,
) -> &mut RebuildColumnSpec {
    if let Some(index) = columns.iter().position(|column| column.name == name) {
        return &mut columns[index];
    }

    columns.push(RebuildColumnSpec {
        name,
        data_type: None,
        not_null: None,
        default: None,
    });
    columns
        .last_mut()
        .expect("rebuild column vector must contain inserted element")
}
