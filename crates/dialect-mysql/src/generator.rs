use std::fmt::Write as _;

use stateql_core::{
    CheckConstraint, Column, ColumnChange, ColumnPosition, DataType, DiffOp, Expr, ForeignKey,
    ForeignKeyAction, GenerateError, GeneratedColumn, Ident, Identity, IndexDef, IndexOwner,
    Partition, PartitionBound, PartitionElement, PartitionStrategy, PrimaryKey, QualifiedName,
    Result, SchemaObject, Statement, Value,
};

use crate::to_sql;

const GENERATOR_TARGET: &str = "mysql ddl generator";
const MYSQL_SUBPARTITIONS_KEY: &str = "mysql.subpartitions";

pub(crate) fn generate_ddl(dialect_name: &str, ops: &[DiffOp]) -> Result<Vec<Statement>> {
    let mut statements = Vec::new();
    let mut index = 0usize;

    while index < ops.len() {
        if let Some((statement, consumed)) = optimize_drop_create_view(dialect_name, &ops[index..])?
        {
            statements.push(statement);
            index += consumed;
            continue;
        }

        if let Some(table) = table_scoped_target(&ops[index]).cloned() {
            let start = index;
            index += 1;

            while let Some(next_op) = ops.get(index) {
                if table_scoped_target(next_op) == Some(&table) {
                    index += 1;
                } else {
                    break;
                }
            }

            emit_table_batch(dialect_name, &table, &ops[start..index], &mut statements)?;
            continue;
        }

        emit_non_table_op(dialect_name, &ops[index], &mut statements)?;
        index += 1;
    }

    Ok(statements)
}

fn optimize_drop_create_view(
    dialect_name: &str,
    ops: &[DiffOp],
) -> Result<Option<(Statement, usize)>> {
    if ops.len() < 2 {
        return Ok(None);
    }

    let (DiffOp::DropView(dropped_name), DiffOp::CreateView(view)) = (&ops[0], &ops[1]) else {
        return Ok(None);
    };

    if dropped_name != &view.name {
        return Ok(None);
    }

    let sql = render_object_sql(dialect_name, &ops[1], &SchemaObject::View(view.clone()))?;
    Ok(Some((
        sql_statement(promote_to_create_or_replace(&sql), true),
        2,
    )))
}

fn promote_to_create_or_replace(sql: &str) -> String {
    let trimmed = sql.trim().trim_end_matches(';').trim();
    if let Some(rest) = trimmed.strip_prefix("CREATE ") {
        return format!("CREATE OR REPLACE {rest};");
    }
    format!("CREATE OR REPLACE {trimmed};")
}

fn table_scoped_target(op: &DiffOp) -> Option<&QualifiedName> {
    match op {
        DiffOp::AddColumn { table, .. }
        | DiffOp::DropColumn { table, .. }
        | DiffOp::AlterColumn { table, .. }
        | DiffOp::RenameColumn { table, .. }
        | DiffOp::AddForeignKey { table, .. }
        | DiffOp::DropForeignKey { table, .. }
        | DiffOp::AddCheck { table, .. }
        | DiffOp::DropCheck { table, .. }
        | DiffOp::SetPrimaryKey { table, .. }
        | DiffOp::DropPrimaryKey { table }
        | DiffOp::AddPartition { table, .. }
        | DiffOp::DropPartition { table, .. }
        | DiffOp::AlterTableOptions { table, .. } => Some(table),
        DiffOp::AddIndex(IndexDef { owner, .. })
        | DiffOp::DropIndex { owner, .. }
        | DiffOp::RenameIndex { owner, .. } => match owner {
            IndexOwner::Table(table) => Some(table),
            IndexOwner::View(_) | IndexOwner::MaterializedView(_) => None,
        },
        _ => None,
    }
}

fn emit_table_batch(
    dialect_name: &str,
    table: &QualifiedName,
    ops: &[DiffOp],
    out: &mut Vec<Statement>,
) -> Result<()> {
    let mut pre_pk_statements = Vec::new();
    let mut pk_statements = Vec::new();
    let mut merged_alter_columns = Vec::new();

    for op in ops {
        match op {
            DiffOp::AddColumn {
                table: target_table,
                column,
                position,
            } => {
                ensure_same_table(op, table, target_table, dialect_name)?;
                pre_pk_statements.push(sql_statement(
                    render_add_column(target_table, column, position),
                    true,
                ));
            }
            DiffOp::DropColumn {
                table: target_table,
                column,
            } => {
                ensure_same_table(op, table, target_table, dialect_name)?;
                pre_pk_statements.push(sql_statement(
                    format!(
                        "ALTER TABLE {} DROP COLUMN {};",
                        to_sql::render_qualified_name(target_table),
                        to_sql::render_ident(column)
                    ),
                    true,
                ));
            }
            DiffOp::AlterColumn {
                table: target_table,
                column,
                changes,
            } => {
                ensure_same_table(op, table, target_table, dialect_name)?;
                if changes.is_empty() {
                    return Err(unsupported_diff_op(
                        dialect_name,
                        op,
                        "ALTER COLUMN requires at least one change",
                    ));
                }
                let merged = upsert_column_change(&mut merged_alter_columns, column.clone());
                for change in changes {
                    merged.apply(change);
                }
            }
            DiffOp::RenameColumn {
                table: target_table,
                from,
                to,
            } => {
                ensure_same_table(op, table, target_table, dialect_name)?;
                pre_pk_statements.push(sql_statement(
                    format!(
                        "ALTER TABLE {} RENAME COLUMN {} TO {};",
                        to_sql::render_qualified_name(target_table),
                        to_sql::render_ident(from),
                        to_sql::render_ident(to)
                    ),
                    true,
                ));
            }
            DiffOp::AddIndex(index) => {
                ensure_same_owner_table(op, table, &index.owner, dialect_name)?;
                pre_pk_statements.push(sql_statement(
                    render_object_sql(dialect_name, op, &SchemaObject::Index(index.clone()))?,
                    true,
                ));
            }
            DiffOp::DropIndex {
                owner,
                name: index_name,
            } => {
                ensure_same_owner_table(op, table, owner, dialect_name)?;
                pre_pk_statements.push(sql_statement(
                    format!(
                        "DROP INDEX {} ON {};",
                        to_sql::render_ident(index_name),
                        to_sql::render_qualified_name(table)
                    ),
                    true,
                ));
            }
            DiffOp::RenameIndex { owner, from, to } => {
                ensure_same_owner_table(op, table, owner, dialect_name)?;
                pre_pk_statements.push(sql_statement(
                    format!(
                        "ALTER TABLE {} RENAME INDEX {} TO {};",
                        to_sql::render_qualified_name(table),
                        to_sql::render_ident(from),
                        to_sql::render_ident(to)
                    ),
                    true,
                ));
            }
            DiffOp::AddForeignKey {
                table: target_table,
                fk,
            } => {
                ensure_same_table(op, table, target_table, dialect_name)?;
                pre_pk_statements.push(sql_statement(
                    render_add_foreign_key(target_table, fk, dialect_name, op)?,
                    true,
                ));
            }
            DiffOp::DropForeignKey {
                table: target_table,
                name,
            } => {
                ensure_same_table(op, table, target_table, dialect_name)?;
                pre_pk_statements.push(sql_statement(
                    format!(
                        "ALTER TABLE {} DROP FOREIGN KEY {};",
                        to_sql::render_qualified_name(target_table),
                        to_sql::render_ident(name)
                    ),
                    true,
                ));
            }
            DiffOp::AddCheck {
                table: target_table,
                check,
            } => {
                ensure_same_table(op, table, target_table, dialect_name)?;
                pre_pk_statements.push(sql_statement(
                    render_add_check(target_table, check, dialect_name, op)?,
                    true,
                ));
            }
            DiffOp::DropCheck {
                table: target_table,
                name,
            } => {
                ensure_same_table(op, table, target_table, dialect_name)?;
                pre_pk_statements.push(sql_statement(
                    format!(
                        "ALTER TABLE {} DROP CHECK {};",
                        to_sql::render_qualified_name(target_table),
                        to_sql::render_ident(name)
                    ),
                    true,
                ));
            }
            DiffOp::SetPrimaryKey {
                table: target_table,
                pk,
            } => {
                ensure_same_table(op, table, target_table, dialect_name)?;
                pk_statements.push(sql_statement(
                    render_set_primary_key(target_table, pk),
                    true,
                ));
            }
            DiffOp::DropPrimaryKey {
                table: target_table,
            } => {
                ensure_same_table(op, table, target_table, dialect_name)?;
                pk_statements.push(sql_statement(
                    format!(
                        "ALTER TABLE {} DROP PRIMARY KEY;",
                        to_sql::render_qualified_name(target_table)
                    ),
                    true,
                ));
            }
            DiffOp::AddPartition {
                table: target_table,
                partition,
            } => {
                ensure_same_table(op, table, target_table, dialect_name)?;
                pre_pk_statements.push(sql_statement(
                    render_add_partition(target_table, partition),
                    true,
                ));
            }
            DiffOp::DropPartition {
                table: target_table,
                name,
            } => {
                ensure_same_table(op, table, target_table, dialect_name)?;
                pre_pk_statements.push(sql_statement(
                    format!(
                        "ALTER TABLE {} DROP PARTITION {};",
                        to_sql::render_qualified_name(target_table),
                        to_sql::render_ident(name)
                    ),
                    true,
                ));
            }
            DiffOp::AlterTableOptions { .. } => {
                return Err(unsupported_diff_op(dialect_name, op, GENERATOR_TARGET));
            }
            _ => return Err(unsupported_diff_op(dialect_name, op, GENERATOR_TARGET)),
        }
    }

    let mut regular_change_column = Vec::new();
    let mut auto_increment_change_column = Vec::new();
    for merged in merged_alter_columns {
        let statement = sql_statement(merged.render_change_column(table, dialect_name)?, true);
        if merged.has_auto_increment_change() {
            auto_increment_change_column.push(statement);
        } else {
            regular_change_column.push(statement);
        }
    }

    out.extend(pre_pk_statements);
    out.extend(regular_change_column);
    out.extend(pk_statements);
    out.extend(auto_increment_change_column);
    Ok(())
}

fn emit_non_table_op(dialect_name: &str, op: &DiffOp, out: &mut Vec<Statement>) -> Result<()> {
    match op {
        DiffOp::CreateTable(table) => {
            out.push(sql_statement(
                render_object_sql(dialect_name, op, &SchemaObject::Table(table.clone()))?,
                true,
            ));
        }
        DiffOp::DropTable(name) => {
            out.push(sql_statement(
                format!("DROP TABLE {};", to_sql::render_qualified_name(name)),
                true,
            ));
        }
        DiffOp::RenameTable { from, to } => {
            out.push(sql_statement(
                format!(
                    "RENAME TABLE {} TO {};",
                    to_sql::render_qualified_name(from),
                    to_sql::render_qualified_name(to)
                ),
                true,
            ));
        }
        DiffOp::CreateView(view) => {
            out.push(sql_statement(
                render_object_sql(dialect_name, op, &SchemaObject::View(view.clone()))?,
                true,
            ));
        }
        DiffOp::DropView(name) => {
            out.push(sql_statement(
                format!("DROP VIEW {};", to_sql::render_qualified_name(name)),
                true,
            ));
        }
        DiffOp::CreateTrigger(trigger) => {
            out.push(sql_statement(
                render_object_sql(dialect_name, op, &SchemaObject::Trigger(trigger.clone()))?,
                true,
            ));
        }
        DiffOp::DropTrigger { name, .. } => {
            out.push(sql_statement(
                format!("DROP TRIGGER {};", to_sql::render_qualified_name(name)),
                true,
            ));
        }
        DiffOp::CreateFunction(function) => {
            out.push(sql_statement(
                render_object_sql(dialect_name, op, &SchemaObject::Function(function.clone()))?,
                true,
            ));
        }
        DiffOp::DropFunction(name) => {
            out.push(sql_statement(
                format!("DROP FUNCTION {};", to_sql::render_qualified_name(name)),
                true,
            ));
        }
        _ => return Err(unsupported_diff_op(dialect_name, op, GENERATOR_TARGET)),
    }

    Ok(())
}

fn ensure_same_table(
    op: &DiffOp,
    expected: &QualifiedName,
    actual: &QualifiedName,
    dialect_name: &str,
) -> Result<()> {
    if expected == actual {
        return Ok(());
    }
    Err(unsupported_diff_op(
        dialect_name,
        op,
        "table batch contained mixed table targets",
    ))
}

fn ensure_same_owner_table(
    op: &DiffOp,
    expected: &QualifiedName,
    owner: &IndexOwner,
    dialect_name: &str,
) -> Result<()> {
    let IndexOwner::Table(actual) = owner else {
        return Err(unsupported_diff_op(
            dialect_name,
            op,
            "mysql indexes must be table-scoped",
        ));
    };

    ensure_same_table(op, expected, actual, dialect_name)
}

fn upsert_column_change(
    changes: &mut Vec<MergedColumnChange>,
    column: Ident,
) -> &mut MergedColumnChange {
    if let Some(index) = changes.iter().position(|entry| entry.column == column) {
        return &mut changes[index];
    }

    changes.push(MergedColumnChange::new(column));
    changes
        .last_mut()
        .expect("entry was just pushed into merged column list")
}

fn render_add_column(
    table: &QualifiedName,
    column: &Column,
    position: &Option<ColumnPosition>,
) -> String {
    let mut sql = format!(
        "ALTER TABLE {} ADD COLUMN {}",
        to_sql::render_qualified_name(table),
        render_column_definition(column)
    );
    if let Some(position) = position {
        match position {
            ColumnPosition::First => sql.push_str(" FIRST"),
            ColumnPosition::After(column_name) => {
                write!(sql, " AFTER {}", to_sql::render_ident(column_name))
                    .expect("writing to String should not fail");
            }
        }
    }
    sql.push(';');
    sql
}

fn render_set_primary_key(table: &QualifiedName, pk: &PrimaryKey) -> String {
    format!(
        "ALTER TABLE {} ADD PRIMARY KEY ({});",
        to_sql::render_qualified_name(table),
        pk.columns
            .iter()
            .map(to_sql::render_ident)
            .collect::<Vec<_>>()
            .join(", ")
    )
}

fn render_add_foreign_key(
    table: &QualifiedName,
    fk: &ForeignKey,
    dialect_name: &str,
    op: &DiffOp,
) -> Result<String> {
    if fk.deferrable.is_some() {
        return Err(unsupported_diff_op(
            dialect_name,
            op,
            "mysql does not support DEFERRABLE foreign keys",
        ));
    }

    let mut sql = format!("ALTER TABLE {} ADD ", to_sql::render_qualified_name(table));
    if let Some(name) = &fk.name {
        write!(sql, "CONSTRAINT {} ", to_sql::render_ident(name))
            .expect("writing to String should not fail");
    }
    write!(
        sql,
        "FOREIGN KEY ({}) REFERENCES {} ({})",
        fk.columns
            .iter()
            .map(to_sql::render_ident)
            .collect::<Vec<_>>()
            .join(", "),
        to_sql::render_qualified_name(&fk.referenced_table),
        fk.referenced_columns
            .iter()
            .map(to_sql::render_ident)
            .collect::<Vec<_>>()
            .join(", ")
    )
    .expect("writing to String should not fail");

    if let Some(action) = fk.on_delete {
        write!(sql, " ON DELETE {}", render_foreign_key_action(action))
            .expect("writing to String should not fail");
    }
    if let Some(action) = fk.on_update {
        write!(sql, " ON UPDATE {}", render_foreign_key_action(action))
            .expect("writing to String should not fail");
    }
    sql.push(';');
    Ok(sql)
}

fn render_add_check(
    table: &QualifiedName,
    check: &CheckConstraint,
    dialect_name: &str,
    op: &DiffOp,
) -> Result<String> {
    if check.no_inherit {
        return Err(unsupported_diff_op(
            dialect_name,
            op,
            "mysql does not support CHECK NO INHERIT",
        ));
    }

    let mut sql = format!("ALTER TABLE {} ADD ", to_sql::render_qualified_name(table));
    if let Some(name) = &check.name {
        write!(sql, "CONSTRAINT {} ", to_sql::render_ident(name))
            .expect("writing to String should not fail");
    }
    write!(sql, "CHECK ({})", to_sql::render_expr(&check.expr))
        .expect("writing to String should not fail");
    sql.push(';');
    Ok(sql)
}

fn render_add_partition(table: &QualifiedName, partition: &Partition) -> String {
    let mut sql = format!(
        "ALTER TABLE {} PARTITION BY {} ({})",
        to_sql::render_qualified_name(table),
        render_partition_strategy(&partition.strategy),
        partition
            .columns
            .iter()
            .map(to_sql::render_ident)
            .collect::<Vec<_>>()
            .join(", ")
    );

    if !partition.partitions.is_empty() {
        let elements = partition
            .partitions
            .iter()
            .map(render_partition_element)
            .collect::<Vec<_>>()
            .join(", ");
        write!(sql, " ({elements})").expect("writing to String should not fail");
    }

    sql.push(';');
    sql
}

fn render_partition_strategy(strategy: &PartitionStrategy) -> &'static str {
    match strategy {
        PartitionStrategy::Range => "RANGE",
        PartitionStrategy::List => "LIST",
        PartitionStrategy::Hash => "HASH",
        PartitionStrategy::Key => "KEY",
    }
}

fn render_partition_element(element: &PartitionElement) -> String {
    let mut sql = format!("PARTITION {}", to_sql::render_ident(&element.name));
    if let Some(bound) = &element.bound {
        match bound {
            PartitionBound::LessThan(values) => {
                write!(
                    sql,
                    " VALUES LESS THAN ({})",
                    values
                        .iter()
                        .map(to_sql::render_expr)
                        .collect::<Vec<_>>()
                        .join(", ")
                )
                .expect("writing to String should not fail");
            }
            PartitionBound::In(values) => {
                write!(
                    sql,
                    " VALUES IN ({})",
                    values
                        .iter()
                        .map(to_sql::render_expr)
                        .collect::<Vec<_>>()
                        .join(", ")
                )
                .expect("writing to String should not fail");
            }
            PartitionBound::FromTo { from, to } => {
                write!(
                    sql,
                    " VALUES FROM ({}) TO ({})",
                    from.iter()
                        .map(to_sql::render_expr)
                        .collect::<Vec<_>>()
                        .join(", "),
                    to.iter()
                        .map(to_sql::render_expr)
                        .collect::<Vec<_>>()
                        .join(", ")
                )
                .expect("writing to String should not fail");
            }
            PartitionBound::MaxValue => sql.push_str(" VALUES LESS THAN (MAXVALUE)"),
        }
    }

    if let Some(Value::String(subpartitions)) = element.extra.get(MYSQL_SUBPARTITIONS_KEY)
        && !subpartitions.trim().is_empty()
    {
        write!(sql, " {}", subpartitions.trim()).expect("writing to String should not fail");
    }

    sql
}

fn render_foreign_key_action(action: ForeignKeyAction) -> &'static str {
    match action {
        ForeignKeyAction::NoAction => "NO ACTION",
        ForeignKeyAction::Restrict => "RESTRICT",
        ForeignKeyAction::Cascade => "CASCADE",
        ForeignKeyAction::SetNull => "SET NULL",
        ForeignKeyAction::SetDefault => "SET DEFAULT",
    }
}

fn render_column_definition(column: &Column) -> String {
    let mut sql = format!(
        "{} {}",
        to_sql::render_ident(&column.name),
        to_sql::render_data_type(&column.data_type)
    );

    if column.not_null {
        sql.push_str(" NOT NULL");
    }
    if let Some(default) = &column.default {
        write!(sql, " DEFAULT {}", to_sql::render_expr(default))
            .expect("writing to String should not fail");
    }
    if column
        .identity
        .as_ref()
        .is_some_and(identity_is_auto_increment)
        || column
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
            to_sql::render_expr(&generated.expr)
        )
        .expect("writing to String should not fail");
    }
    if let Some(collation) = &column.collation {
        write!(sql, " COLLATE {}", collation.trim()).expect("writing to String should not fail");
    }

    sql
}

fn identity_is_auto_increment(identity: &Identity) -> bool {
    identity.always
        || identity.start.is_some()
        || identity.increment.is_some()
        || identity.min_value.is_some()
        || identity.max_value.is_some()
        || identity.cache.is_some()
        || identity.cycle
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

fn render_object_sql(dialect_name: &str, op: &DiffOp, object: &SchemaObject) -> Result<String> {
    match to_sql::render_object(dialect_name, object) {
        Ok(sql) => Ok(sql),
        Err(stateql_core::Error::Generate(GenerateError::UnsupportedDiffOp { target, .. })) => {
            Err(unsupported_diff_op(dialect_name, op, target))
        }
        Err(other) => Err(other),
    }
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
struct MergedColumnChange {
    column: Ident,
    data_type: Option<DataType>,
    not_null: Option<bool>,
    default: Option<Option<Expr>>,
    identity: Option<Option<Identity>>,
    generated: Option<Option<GeneratedColumn>>,
    collation: Option<Option<String>>,
}

impl MergedColumnChange {
    fn new(column: Ident) -> Self {
        Self {
            column,
            data_type: None,
            not_null: None,
            default: None,
            identity: None,
            generated: None,
            collation: None,
        }
    }

    fn apply(&mut self, change: &ColumnChange) {
        match change {
            ColumnChange::SetType(data_type) => self.data_type = Some(data_type.clone()),
            ColumnChange::SetNotNull(not_null) => self.not_null = Some(*not_null),
            ColumnChange::SetDefault(default) => self.default = Some(default.clone()),
            ColumnChange::SetIdentity(identity) => self.identity = Some(identity.clone()),
            ColumnChange::SetGenerated(generated) => self.generated = Some(generated.clone()),
            ColumnChange::SetCollation(collation) => self.collation = Some(collation.clone()),
        }
    }

    fn has_auto_increment_change(&self) -> bool {
        self.identity
            .as_ref()
            .is_some_and(|value| value.as_ref().is_some_and(identity_is_auto_increment))
    }

    fn render_change_column(&self, table: &QualifiedName, dialect_name: &str) -> Result<String> {
        let data_type = self
            .data_type
            .clone()
            .ok_or_else(|| GenerateError::UnsupportedDiffOp {
                diff_op: "AlterColumn".to_string(),
                target: format!("{GENERATOR_TARGET}: CHANGE COLUMN requires SetType"),
                dialect: dialect_name.to_string(),
            })?;

        let mut sql = format!(
            "ALTER TABLE {} CHANGE COLUMN {} {} {}",
            to_sql::render_qualified_name(table),
            to_sql::render_ident(&self.column),
            to_sql::render_ident(&self.column),
            to_sql::render_data_type(&data_type)
        );

        match self.not_null {
            Some(true) => sql.push_str(" NOT NULL"),
            Some(false) => sql.push_str(" NULL"),
            None => {}
        }

        if let Some(default) = &self.default {
            match default {
                Some(expr) => {
                    write!(sql, " DEFAULT {}", to_sql::render_expr(expr))
                        .expect("writing to String should not fail");
                }
                None => sql.push_str(" DEFAULT NULL"),
            }
        }

        if self
            .identity
            .as_ref()
            .is_some_and(|identity| identity.as_ref().is_some_and(identity_is_auto_increment))
        {
            sql.push_str(" AUTO_INCREMENT");
        }

        if let Some(Some(generated)) = &self.generated {
            let storage = if generated.stored {
                "STORED"
            } else {
                "VIRTUAL"
            };
            write!(
                sql,
                " GENERATED ALWAYS AS ({}) {storage}",
                to_sql::render_expr(&generated.expr)
            )
            .expect("writing to String should not fail");
        }

        if let Some(Some(collation)) = &self.collation {
            write!(sql, " COLLATE {}", collation.trim())
                .expect("writing to String should not fail");
        }

        sql.push(';');
        Ok(sql)
    }
}
