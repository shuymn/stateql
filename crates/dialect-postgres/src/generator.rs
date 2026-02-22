use std::fmt::Write;

use stateql_core::{
    BinaryOperator, CheckConstraint, CheckOption, Column, ColumnChange, ColumnPosition, Comment,
    CommentTarget, ComparisonOp, DataType, Deferrable, DiffOp, Domain, DomainChange,
    EnumValuePosition, ExclusionConstraint, ExclusionElement, Expr, ForeignKey, ForeignKeyAction,
    Function, FunctionParam, FunctionParamMode, FunctionSecurity, GenerateError, GeneratedColumn,
    Ident, Identity, IndexDef, IndexOwner, IsTest, Literal, MaterializedView, NullsOrder,
    Partition, PartitionBound, PartitionElement, PartitionStrategy, Policy, PolicyCommand,
    PrimaryKey, Privilege, PrivilegeObject, PrivilegeOp, QualifiedName, Result, SchemaDef,
    Sequence, SequenceChange, SetQuantifier, SortOrder, Statement, SubQuery, Table, TableOptions,
    Trigger, TriggerEvent, TriggerForEach, TriggerTiming, TypeChange, TypeDef, TypeKind,
    UnaryOperator, Value, View, ViewSecurity, Volatility,
};

use crate::extra_keys;

pub(crate) fn generate_ddl(dialect_name: &str, ops: &[DiffOp]) -> Result<Vec<Statement>> {
    let mut statements = Vec::new();
    let mut index = 0usize;

    while index < ops.len() {
        if let Some((statement, consumed)) = optimize_drop_create_view(&ops[index..])? {
            statements.push(statement);
            index += consumed;
            continue;
        }

        emit_op(dialect_name, &ops[index], &mut statements)?;
        index += 1;
    }

    Ok(statements)
}

fn optimize_drop_create_view(ops: &[DiffOp]) -> Result<Option<(Statement, usize)>> {
    if ops.len() < 2 {
        return Ok(None);
    }

    let (DiffOp::DropView(dropped_name), DiffOp::CreateView(view)) = (&ops[0], &ops[1]) else {
        return Ok(None);
    };

    if dropped_name != &view.name {
        return Ok(None);
    }

    if !is_create_or_replace_view_compatible(view) {
        return Ok(None);
    }

    let sql = render_create_view(view, true)?;
    Ok(Some((sql_statement(sql, true), 2)))
}

fn is_create_or_replace_view_compatible(view: &View) -> bool {
    // Without old-view metadata in DiffOp, keep optimization conservative.
    // Column list / policy options imply shape-sensitive behavior, so fallback
    // to DROP + CREATE in those cases.
    view.columns.is_empty() && view.check_option.is_none() && view.security.is_none()
}

fn emit_op(dialect_name: &str, op: &DiffOp, out: &mut Vec<Statement>) -> Result<()> {
    match op {
        DiffOp::CreateTable(table) => {
            out.push(sql_statement(render_create_table(table)?, true));
        }
        DiffOp::DropTable(name) => {
            out.push(sql_statement(
                format!("DROP TABLE {}", render_qualified_name(name)),
                true,
            ));
        }
        DiffOp::RenameTable { from, to } => {
            for sql in render_rename_table(from, to, dialect_name, op)? {
                out.push(sql_statement(sql, true));
            }
        }
        DiffOp::AddColumn {
            table,
            column,
            position,
        } => {
            let mut sql = format!(
                "ALTER TABLE {} ADD COLUMN {}",
                render_qualified_name(table),
                render_column_definition(column)
            );
            if let Some(position) = position {
                write!(
                    sql,
                    " /* requested position: {} */",
                    render_column_position(position)
                )
                .expect("writing to String should not fail");
            }
            out.push(sql_statement(sql, true));
        }
        DiffOp::DropColumn { table, column } => {
            out.push(sql_statement(
                format!(
                    "ALTER TABLE {} DROP COLUMN {}",
                    render_qualified_name(table),
                    render_ident(column)
                ),
                true,
            ));
        }
        DiffOp::AlterColumn {
            table,
            column,
            changes,
        } => {
            for change in changes {
                out.push(sql_statement(
                    render_alter_column_change(table, column, change),
                    true,
                ));
            }
        }
        DiffOp::RenameColumn { table, from, to } => {
            out.push(sql_statement(
                format!(
                    "ALTER TABLE {} RENAME COLUMN {} TO {}",
                    render_qualified_name(table),
                    render_ident(from),
                    render_ident(to)
                ),
                true,
            ));
        }
        DiffOp::AddIndex(index) => {
            let sql = render_add_index(index, dialect_name, op)?;
            out.push(sql_statement(sql, !index.concurrent));
        }
        DiffOp::DropIndex { owner, name } => {
            let qualified = render_owner_scoped_name(owner, name);
            out.push(sql_statement(format!("DROP INDEX {}", qualified), true));
        }
        DiffOp::RenameIndex { owner, from, to } => {
            out.push(sql_statement(
                format!(
                    "ALTER INDEX {} RENAME TO {}",
                    render_owner_scoped_name(owner, from),
                    render_ident(to)
                ),
                true,
            ));
        }
        DiffOp::AddForeignKey { table, fk } => {
            out.push(sql_statement(render_add_foreign_key(table, fk), true));
        }
        DiffOp::DropForeignKey { table, name } => {
            out.push(sql_statement(
                format!(
                    "ALTER TABLE {} DROP CONSTRAINT {}",
                    render_qualified_name(table),
                    render_ident(name)
                ),
                true,
            ));
        }
        DiffOp::AddCheck { table, check } => {
            out.push(sql_statement(render_add_check(table, check), true));
        }
        DiffOp::DropCheck { table, name } => {
            out.push(sql_statement(
                format!(
                    "ALTER TABLE {} DROP CONSTRAINT {}",
                    render_qualified_name(table),
                    render_ident(name)
                ),
                true,
            ));
        }
        DiffOp::AddExclusion { table, exclusion } => {
            out.push(sql_statement(render_add_exclusion(table, exclusion), true));
        }
        DiffOp::DropExclusion { table, name } => {
            out.push(sql_statement(
                format!(
                    "ALTER TABLE {} DROP CONSTRAINT {}",
                    render_qualified_name(table),
                    render_ident(name)
                ),
                true,
            ));
        }
        DiffOp::SetPrimaryKey { table, pk } => {
            out.push(sql_statement(render_set_primary_key(table, pk), true));
        }
        DiffOp::DropPrimaryKey { table } => {
            out.push(sql_statement(render_drop_primary_key(table), true));
        }
        DiffOp::AddPartition { table, partition } => {
            for statement in render_add_partition(table, partition) {
                out.push(sql_statement(statement, true));
            }
        }
        DiffOp::DropPartition { table, name } => {
            out.push(sql_statement(render_drop_partition(table, name), true));
        }
        DiffOp::CreateView(view) => {
            out.push(sql_statement(render_create_view(view, false)?, true));
        }
        DiffOp::DropView(name) => {
            out.push(sql_statement(
                format!("DROP VIEW {}", render_qualified_name(name)),
                true,
            ));
        }
        DiffOp::CreateMaterializedView(view) => {
            out.push(sql_statement(render_create_materialized_view(view), true));
        }
        DiffOp::DropMaterializedView(name) => {
            out.push(sql_statement(
                format!("DROP MATERIALIZED VIEW {}", render_qualified_name(name)),
                true,
            ));
        }
        DiffOp::CreateSequence(sequence) => {
            out.push(sql_statement(render_create_sequence(sequence), true));
        }
        DiffOp::DropSequence(name) => {
            out.push(sql_statement(
                format!("DROP SEQUENCE {}", render_qualified_name(name)),
                true,
            ));
        }
        DiffOp::AlterSequence { name, changes } => {
            out.push(sql_statement(render_alter_sequence(name, changes), true));
        }
        DiffOp::CreateTrigger(trigger) => {
            out.push(sql_statement(render_create_trigger(trigger), true));
        }
        DiffOp::DropTrigger { name, table } => {
            let table = table.as_ref().ok_or_else(|| {
                unsupported_diff_op(dialect_name, op, "trigger drop requires table context")
            })?;
            out.push(sql_statement(
                format!(
                    "DROP TRIGGER {} ON {}",
                    render_ident(&name.name),
                    render_qualified_name(table)
                ),
                true,
            ));
        }
        DiffOp::CreateFunction(function) => {
            out.push(sql_statement(render_create_function(function), true));
        }
        DiffOp::DropFunction(name) => {
            out.push(sql_statement(
                format!("DROP FUNCTION {}", render_qualified_name(name)),
                true,
            ));
        }
        DiffOp::CreateType(ty) => {
            out.push(sql_statement(render_create_type(ty), true));
        }
        DiffOp::DropType(name) => {
            out.push(sql_statement(
                format!("DROP TYPE {}", render_qualified_name(name)),
                true,
            ));
        }
        DiffOp::AlterType { name, change } => {
            out.push(sql_statement(render_alter_type(name, change), true));
        }
        DiffOp::CreateDomain(domain) => {
            out.push(sql_statement(render_create_domain(domain), true));
        }
        DiffOp::DropDomain(name) => {
            out.push(sql_statement(
                format!("DROP DOMAIN {}", render_qualified_name(name)),
                true,
            ));
        }
        DiffOp::AlterDomain { name, change } => {
            out.push(sql_statement(render_alter_domain(name, change), true));
        }
        DiffOp::CreateExtension(extension) => {
            out.push(sql_statement(render_create_extension(extension), true));
        }
        DiffOp::DropExtension(name) => {
            out.push(sql_statement(
                format!("DROP EXTENSION {}", render_qualified_name(name)),
                true,
            ));
        }
        DiffOp::CreateSchema(schema) => {
            out.push(sql_statement(render_create_schema(schema), true));
        }
        DiffOp::DropSchema(name) => {
            out.push(sql_statement(
                format!("DROP SCHEMA {}", render_qualified_name(name)),
                true,
            ));
        }
        DiffOp::SetComment(comment) => {
            out.push(sql_statement(render_set_comment(comment), true));
        }
        DiffOp::DropComment { target } => {
            out.push(sql_statement(render_drop_comment(target), true));
        }
        DiffOp::Grant(privilege) => {
            out.push(sql_statement(
                render_grant(privilege, dialect_name, op)?,
                true,
            ));
        }
        DiffOp::Revoke(privilege) => {
            out.push(sql_statement(
                render_revoke(privilege, dialect_name, op)?,
                true,
            ));
        }
        DiffOp::CreatePolicy(policy) => {
            out.push(sql_statement(render_create_policy(policy), true));
        }
        DiffOp::DropPolicy { name, table } => {
            out.push(sql_statement(
                format!(
                    "DROP POLICY {} ON {}",
                    render_ident(name),
                    render_qualified_name(table)
                ),
                true,
            ));
        }
        DiffOp::AlterTableOptions { table, options } => {
            out.push(sql_statement(
                render_alter_table_options(table, options, dialect_name, op)?,
                true,
            ));
        }
    }

    Ok(())
}

fn render_rename_table(
    from: &QualifiedName,
    to: &QualifiedName,
    dialect_name: &str,
    op: &DiffOp,
) -> Result<Vec<String>> {
    let mut sql = Vec::new();

    if from.name != to.name {
        sql.push(format!(
            "ALTER TABLE {} RENAME TO {}",
            render_qualified_name(from),
            render_ident(&to.name)
        ));
    }

    if from.schema != to.schema {
        let target_schema = to.schema.as_ref().ok_or_else(|| {
            unsupported_diff_op(dialect_name, op, "target schema must be explicit")
        })?;
        let current_name = if from.name != to.name {
            &to.name
        } else {
            &from.name
        };
        let intermediate = QualifiedName {
            schema: from.schema.clone(),
            name: current_name.clone(),
        };
        sql.push(format!(
            "ALTER TABLE {} SET SCHEMA {}",
            render_qualified_name(&intermediate),
            render_ident(target_schema)
        ));
    }

    if sql.is_empty() {
        return Err(unsupported_diff_op(
            dialect_name,
            op,
            "rename table op has no effective change",
        ));
    }

    Ok(sql)
}

fn render_create_table(table: &Table) -> Result<String> {
    let mut elements = Vec::new();

    for column in &table.columns {
        elements.push(render_column_definition(column));
    }

    if let Some(primary_key) = &table.primary_key {
        elements.push(render_primary_key_clause(primary_key));
    }

    for foreign_key in &table.foreign_keys {
        elements.push(render_foreign_key_clause(foreign_key));
    }

    for check in &table.checks {
        elements.push(render_check_clause(check));
    }

    for exclusion in &table.exclusions {
        elements.push(render_exclusion_clause(exclusion));
    }

    let mut sql = String::new();
    let if_not_exists = matches!(
        table.options.extra.get(extra_keys::TABLE_IF_NOT_EXISTS),
        Some(Value::Bool(true))
    );

    write!(
        sql,
        "CREATE TABLE {}{} ({})",
        if if_not_exists { "IF NOT EXISTS " } else { "" },
        render_qualified_name(&table.name),
        elements.join(", ")
    )
    .expect("writing to String should not fail");

    if let Some(partition) = &table.partition {
        write!(sql, " PARTITION BY {}", render_partition_key(partition))
            .expect("writing to String should not fail");
    }

    if let Some(Value::String(access_method)) =
        table.options.extra.get(extra_keys::TABLE_ACCESS_METHOD)
    {
        write!(sql, " USING {}", access_method).expect("writing to String should not fail");
    }

    if let Some(Value::String(tablespace)) = table.options.extra.get(extra_keys::TABLESPACE) {
        write!(
            sql,
            " TABLESPACE {}",
            render_ident(&Ident::unquoted(tablespace))
        )
        .expect("writing to String should not fail");
    }

    Ok(sql)
}

fn render_column_definition(column: &Column) -> String {
    let mut sql = format!(
        "{} {}",
        render_ident(&column.name),
        render_data_type(&column.data_type)
    );

    if let Some(collation) = &column.collation {
        write!(
            sql,
            " COLLATE {}",
            render_ident(&Ident::unquoted(collation))
        )
        .expect("writing to String should not fail");
    }

    if let Some(identity) = &column.identity {
        write!(sql, " {}", render_identity(identity)).expect("writing to String should not fail");
    }

    if let Some(generated) = &column.generated {
        write!(sql, " {}", render_generated(generated)).expect("writing to String should not fail");
    }

    if let Some(default_expr) = &column.default {
        write!(sql, " DEFAULT {}", render_expr(default_expr))
            .expect("writing to String should not fail");
    }

    if column.not_null {
        sql.push_str(" NOT NULL");
    }

    sql
}

fn render_primary_key_clause(primary_key: &PrimaryKey) -> String {
    let mut sql = String::new();

    if let Some(name) = &primary_key.name {
        write!(sql, "CONSTRAINT {} ", render_ident(name))
            .expect("writing to String should not fail");
    }

    write!(
        sql,
        "PRIMARY KEY ({})",
        render_ident_list(&primary_key.columns)
    )
    .expect("writing to String should not fail");

    sql
}

fn render_foreign_key_clause(foreign_key: &ForeignKey) -> String {
    let mut sql = String::new();

    if let Some(name) = &foreign_key.name {
        write!(sql, "CONSTRAINT {} ", render_ident(name))
            .expect("writing to String should not fail");
    }

    write!(
        sql,
        "FOREIGN KEY ({}) REFERENCES {} ({})",
        render_ident_list(&foreign_key.columns),
        render_qualified_name(&foreign_key.referenced_table),
        render_ident_list(&foreign_key.referenced_columns)
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
    if let Some(deferrable) = foreign_key.deferrable {
        write!(sql, " {}", render_deferrable(deferrable))
            .expect("writing to String should not fail");
    }

    sql
}

fn render_check_clause(check: &CheckConstraint) -> String {
    let mut sql = String::new();

    if let Some(name) = &check.name {
        write!(sql, "CONSTRAINT {} ", render_ident(name))
            .expect("writing to String should not fail");
    }

    write!(sql, "CHECK ({})", render_expr(&check.expr)).expect("writing to String should not fail");

    if check.no_inherit {
        sql.push_str(" NO INHERIT");
    }

    sql
}

fn render_exclusion_clause(exclusion: &ExclusionConstraint) -> String {
    let mut sql = String::new();

    if let Some(name) = &exclusion.name {
        write!(sql, "CONSTRAINT {} ", render_ident(name))
            .expect("writing to String should not fail");
    }

    write!(
        sql,
        "EXCLUDE USING {} ({})",
        exclusion.index_method,
        exclusion
            .elements
            .iter()
            .map(render_exclusion_element)
            .collect::<Vec<_>>()
            .join(", ")
    )
    .expect("writing to String should not fail");

    if let Some(where_clause) = &exclusion.where_clause {
        write!(sql, " WHERE ({})", render_expr(where_clause))
            .expect("writing to String should not fail");
    }

    if let Some(deferrable) = exclusion.deferrable {
        write!(sql, " {}", render_deferrable(deferrable))
            .expect("writing to String should not fail");
    }

    sql
}

fn render_exclusion_element(element: &ExclusionElement) -> String {
    let mut sql = format!("({}) WITH {}", render_expr(&element.expr), element.operator);

    if let Some(opclass) = &element.opclass {
        write!(sql, " {}", opclass).expect("writing to String should not fail");
    }
    if let Some(order) = element.order {
        write!(sql, " {}", render_sort_order(order)).expect("writing to String should not fail");
    }
    if let Some(nulls) = element.nulls {
        write!(sql, " NULLS {}", render_nulls_order(nulls))
            .expect("writing to String should not fail");
    }

    sql
}

fn render_add_foreign_key(table: &QualifiedName, foreign_key: &ForeignKey) -> String {
    let mut sql = format!(
        "ALTER TABLE {} ADD {}",
        render_qualified_name(table),
        render_foreign_key_clause(foreign_key)
    );

    if let Some(deferrable) = foreign_key.deferrable {
        write!(sql, " {}", render_deferrable(deferrable))
            .expect("writing to String should not fail");
    }

    sql
}

fn render_add_check(table: &QualifiedName, check: &CheckConstraint) -> String {
    format!(
        "ALTER TABLE {} ADD {}",
        render_qualified_name(table),
        render_check_clause(check)
    )
}

fn render_add_exclusion(table: &QualifiedName, exclusion: &ExclusionConstraint) -> String {
    format!(
        "ALTER TABLE {} ADD {}",
        render_qualified_name(table),
        render_exclusion_clause(exclusion)
    )
}

fn render_set_primary_key(table: &QualifiedName, primary_key: &PrimaryKey) -> String {
    format!(
        "ALTER TABLE {} ADD {}",
        render_qualified_name(table),
        render_primary_key_clause(primary_key)
    )
}

fn render_drop_primary_key(table: &QualifiedName) -> String {
    let default_name = Ident::unquoted(format!("{}_pkey", table.name.value));
    format!(
        "ALTER TABLE {} DROP CONSTRAINT {}",
        render_qualified_name(table),
        render_ident(&default_name)
    )
}

fn render_add_partition(table: &QualifiedName, partition: &Partition) -> Vec<String> {
    let mut statements = Vec::new();

    for element in &partition.partitions {
        statements.push(render_create_partition(table, element));
    }

    statements
}

fn render_create_partition(parent: &QualifiedName, element: &PartitionElement) -> String {
    let partition_name = QualifiedName {
        schema: parent.schema.clone(),
        name: element.name.clone(),
    };

    let mut sql = format!(
        "CREATE TABLE {} PARTITION OF {}",
        render_qualified_name(&partition_name),
        render_qualified_name(parent)
    );

    if let Some(bound) = &element.bound {
        write!(sql, " {}", render_partition_bound(bound))
            .expect("writing to String should not fail");
    }

    sql
}

fn render_drop_partition(table: &QualifiedName, name: &Ident) -> String {
    let partition_name = QualifiedName {
        schema: table.schema.clone(),
        name: name.clone(),
    };

    format!("DROP TABLE {}", render_qualified_name(&partition_name))
}

fn render_create_view(view: &View, replace: bool) -> Result<String> {
    if view.query.trim().is_empty() {
        return Err(GenerateError::UnsupportedDiffOp {
            diff_op: "CreateView".to_string(),
            target: "view query must not be empty".to_string(),
            dialect: "postgres".to_string(),
        }
        .into());
    }

    let mut sql = String::new();
    write!(
        sql,
        "CREATE {}VIEW {}",
        if replace { "OR REPLACE " } else { "" },
        render_qualified_name(&view.name)
    )
    .expect("writing to String should not fail");

    if !view.columns.is_empty() {
        write!(sql, " ({})", render_ident_list(&view.columns))
            .expect("writing to String should not fail");
    }

    write!(sql, " AS {}", view.query).expect("writing to String should not fail");

    if let Some(check_option) = view.check_option {
        write!(
            sql,
            " WITH {} CHECK OPTION",
            render_check_option(check_option)
        )
        .expect("writing to String should not fail");
    }

    if let Some(security) = view.security {
        write!(sql, " /* security={} */", render_view_security(security))
            .expect("writing to String should not fail");
    }

    Ok(sql)
}

fn render_create_materialized_view(view: &MaterializedView) -> String {
    let mut sql = format!(
        "CREATE MATERIALIZED VIEW {}",
        render_qualified_name(&view.name)
    );

    if !view.columns.is_empty() {
        let column_names = view
            .columns
            .iter()
            .map(|column| render_ident(&column.name))
            .collect::<Vec<_>>();
        write!(sql, " ({})", column_names.join(", ")).expect("writing to String should not fail");
    }

    write!(sql, " AS {}", view.query).expect("writing to String should not fail");

    if !view.options.extra.is_empty() {
        write!(sql, " {}", render_table_options_comment(&view.options))
            .expect("writing to String should not fail");
    }

    sql
}

fn render_create_sequence(sequence: &Sequence) -> String {
    let mut clauses = Vec::new();

    if let Some(data_type) = &sequence.data_type {
        clauses.push(format!("AS {}", render_data_type(data_type)));
    }
    if let Some(increment) = sequence.increment {
        clauses.push(format!("INCREMENT BY {increment}"));
    }
    if let Some(min_value) = sequence.min_value {
        clauses.push(format!("MINVALUE {min_value}"));
    }
    if let Some(max_value) = sequence.max_value {
        clauses.push(format!("MAXVALUE {max_value}"));
    }
    if let Some(start) = sequence.start {
        clauses.push(format!("START WITH {start}"));
    }
    if let Some(cache) = sequence.cache {
        clauses.push(format!("CACHE {cache}"));
    }
    clauses.push(if sequence.cycle {
        "CYCLE".to_string()
    } else {
        "NO CYCLE".to_string()
    });

    if let Some((owned_table, owned_column)) = &sequence.owned_by {
        clauses.push(format!(
            "OWNED BY {}.{}",
            render_qualified_name(owned_table),
            render_ident(owned_column)
        ));
    }

    format!(
        "CREATE SEQUENCE {} {}",
        render_qualified_name(&sequence.name),
        clauses.join(" ")
    )
}

fn render_alter_sequence(name: &QualifiedName, changes: &[SequenceChange]) -> String {
    let clauses = changes
        .iter()
        .map(render_sequence_change)
        .collect::<Vec<_>>();

    format!(
        "ALTER SEQUENCE {} {}",
        render_qualified_name(name),
        clauses.join(" ")
    )
}

fn render_sequence_change(change: &SequenceChange) -> String {
    match change {
        SequenceChange::SetType(data_type) => format!("AS {}", render_data_type(data_type)),
        SequenceChange::SetIncrement(value) => format!("INCREMENT BY {value}"),
        SequenceChange::SetMinValue(value) => value
            .map(|value| format!("MINVALUE {value}"))
            .unwrap_or_else(|| "NO MINVALUE".to_string()),
        SequenceChange::SetMaxValue(value) => value
            .map(|value| format!("MAXVALUE {value}"))
            .unwrap_or_else(|| "NO MAXVALUE".to_string()),
        SequenceChange::SetStart(value) => format!("START WITH {value}"),
        SequenceChange::SetCache(value) => format!("CACHE {value}"),
        SequenceChange::SetCycle(cycle) => {
            if *cycle {
                "CYCLE".to_string()
            } else {
                "NO CYCLE".to_string()
            }
        }
    }
}

fn render_create_trigger(trigger: &Trigger) -> String {
    let mut sql = format!(
        "CREATE TRIGGER {} {} {} ON {} FOR EACH {}",
        render_ident(&trigger.name.name),
        render_trigger_timing(trigger.timing),
        trigger
            .events
            .iter()
            .map(render_trigger_event)
            .collect::<Vec<_>>()
            .join(" OR "),
        render_qualified_name(&trigger.table),
        render_trigger_for_each(trigger.for_each)
    );

    if let Some(when_clause) = &trigger.when_clause {
        write!(sql, " WHEN ({})", render_expr(when_clause))
            .expect("writing to String should not fail");
    }

    write!(sql, " {}", trigger.body).expect("writing to String should not fail");

    sql
}

fn render_create_function(function: &Function) -> String {
    let mut sql = format!(
        "CREATE FUNCTION {}({}) RETURNS {} LANGUAGE {}",
        render_qualified_name(&function.name),
        function
            .params
            .iter()
            .map(render_function_param)
            .collect::<Vec<_>>()
            .join(", "),
        function
            .return_type
            .as_ref()
            .map(render_data_type)
            .unwrap_or_else(|| "void".to_string()),
        function.language
    );

    if let Some(volatility) = function.volatility {
        write!(sql, " {}", render_volatility(volatility))
            .expect("writing to String should not fail");
    }
    if let Some(security) = function.security {
        write!(sql, " SECURITY {}", render_function_security(security))
            .expect("writing to String should not fail");
    }

    write!(sql, " AS {}", render_dollar_quoted(&function.body))
        .expect("writing to String should not fail");

    sql
}

fn render_function_param(param: &FunctionParam) -> String {
    let mut sql = String::new();

    if let Some(mode) = param.mode {
        write!(sql, "{} ", render_param_mode(mode)).expect("writing to String should not fail");
    }
    if let Some(name) = &param.name {
        write!(sql, "{} ", render_ident(name)).expect("writing to String should not fail");
    }

    write!(sql, "{}", render_data_type(&param.data_type))
        .expect("writing to String should not fail");

    if let Some(default_expr) = &param.default {
        write!(sql, " DEFAULT {}", render_expr(default_expr))
            .expect("writing to String should not fail");
    }

    sql
}

fn render_create_type(ty: &TypeDef) -> String {
    match &ty.kind {
        TypeKind::Enum { labels } => {
            let labels = labels
                .iter()
                .map(|label| quote_string(label))
                .collect::<Vec<_>>()
                .join(", ");
            format!(
                "CREATE TYPE {} AS ENUM ({labels})",
                render_qualified_name(&ty.name)
            )
        }
        TypeKind::Composite { fields } => {
            let fields = fields
                .iter()
                .map(|(name, data_type)| {
                    format!("{} {}", render_ident(name), render_data_type(data_type))
                })
                .collect::<Vec<_>>()
                .join(", ");
            format!(
                "CREATE TYPE {} AS ({fields})",
                render_qualified_name(&ty.name)
            )
        }
        TypeKind::Range { subtype } => format!(
            "CREATE TYPE {} AS RANGE (SUBTYPE = {})",
            render_qualified_name(&ty.name),
            render_data_type(subtype)
        ),
    }
}

fn render_alter_type(name: &QualifiedName, change: &TypeChange) -> String {
    match change {
        TypeChange::AddValue { value, position } => {
            let mut sql = format!(
                "ALTER TYPE {} ADD VALUE {}",
                render_qualified_name(name),
                quote_string(value)
            );
            if let Some(position) = position {
                let clause = match position {
                    EnumValuePosition::Before(existing) => {
                        format!(" BEFORE {}", quote_string(existing))
                    }
                    EnumValuePosition::After(existing) => {
                        format!(" AFTER {}", quote_string(existing))
                    }
                };
                sql.push_str(&clause);
            }
            sql
        }
        TypeChange::RenameValue { from, to } => format!(
            "ALTER TYPE {} RENAME VALUE {} TO {}",
            render_qualified_name(name),
            quote_string(from),
            quote_string(to)
        ),
    }
}

fn render_create_domain(domain: &Domain) -> String {
    let mut sql = format!(
        "CREATE DOMAIN {} AS {}",
        render_qualified_name(&domain.name),
        render_data_type(&domain.data_type)
    );

    if let Some(default_expr) = &domain.default {
        write!(sql, " DEFAULT {}", render_expr(default_expr))
            .expect("writing to String should not fail");
    }
    if domain.not_null {
        sql.push_str(" NOT NULL");
    }
    for check in &domain.checks {
        write!(sql, " {}", render_check_clause(check)).expect("writing to String should not fail");
    }

    sql
}

fn render_alter_domain(name: &QualifiedName, change: &DomainChange) -> String {
    match change {
        DomainChange::SetDefault(default_expr) => default_expr
            .as_ref()
            .map(|expr| {
                format!(
                    "ALTER DOMAIN {} SET DEFAULT {}",
                    render_qualified_name(name),
                    render_expr(expr)
                )
            })
            .unwrap_or_else(|| {
                format!("ALTER DOMAIN {} DROP DEFAULT", render_qualified_name(name))
            }),
        DomainChange::SetNotNull(not_null) => {
            if *not_null {
                format!("ALTER DOMAIN {} SET NOT NULL", render_qualified_name(name))
            } else {
                format!("ALTER DOMAIN {} DROP NOT NULL", render_qualified_name(name))
            }
        }
        DomainChange::AddConstraint {
            name: constraint_name,
            check,
        } => {
            let mut sql = format!("ALTER DOMAIN {} ADD", render_qualified_name(name));
            if let Some(name) = constraint_name {
                write!(sql, " CONSTRAINT {}", render_ident(name))
                    .expect("writing to String should not fail");
            }
            write!(sql, " CHECK ({})", render_expr(check))
                .expect("writing to String should not fail");
            sql
        }
        DomainChange::DropConstraint(constraint_name) => format!(
            "ALTER DOMAIN {} DROP CONSTRAINT {}",
            render_qualified_name(name),
            render_ident(constraint_name)
        ),
    }
}

fn render_create_extension(extension: &stateql_core::Extension) -> String {
    let mut sql = format!("CREATE EXTENSION {}", render_ident(&extension.name));

    let mut clauses = Vec::new();
    if let Some(schema) = &extension.schema {
        clauses.push(format!("SCHEMA {}", render_ident(schema)));
    }
    if let Some(version) = &extension.version {
        clauses.push(format!("VERSION {}", quote_string(version)));
    }

    if !clauses.is_empty() {
        write!(sql, " WITH {}", clauses.join(" ")).expect("writing to String should not fail");
    }

    sql
}

fn render_create_schema(schema: &SchemaDef) -> String {
    format!("CREATE SCHEMA {}", render_ident(&schema.name))
}

fn render_set_comment(comment: &Comment) -> String {
    format!(
        "COMMENT ON {} IS {}",
        render_comment_target(&comment.target),
        comment
            .text
            .as_ref()
            .map(|text| quote_string(text))
            .unwrap_or_else(|| "NULL".to_string())
    )
}

fn render_drop_comment(target: &CommentTarget) -> String {
    format!("COMMENT ON {} IS NULL", render_comment_target(target))
}

fn render_comment_target(target: &CommentTarget) -> String {
    match target {
        CommentTarget::Table(name) => format!("TABLE {}", render_qualified_name(name)),
        CommentTarget::Column { table, column } => {
            format!(
                "COLUMN {}.{}",
                render_qualified_name(table),
                render_ident(column)
            )
        }
        CommentTarget::Index(name) => format!("INDEX {}", render_qualified_name(name)),
        CommentTarget::View(name) => format!("VIEW {}", render_qualified_name(name)),
        CommentTarget::MaterializedView(name) => {
            format!("MATERIALIZED VIEW {}", render_qualified_name(name))
        }
        CommentTarget::Sequence(name) => format!("SEQUENCE {}", render_qualified_name(name)),
        CommentTarget::Trigger(name) => format!("TRIGGER {}", render_qualified_name(name)),
        CommentTarget::Function(name) => format!("FUNCTION {}", render_qualified_name(name)),
        CommentTarget::Type(name) => format!("TYPE {}", render_qualified_name(name)),
        CommentTarget::Domain(name) => format!("DOMAIN {}", render_qualified_name(name)),
        CommentTarget::Extension(name) => format!("EXTENSION {}", render_ident(name)),
        CommentTarget::Schema(name) => format!("SCHEMA {}", render_ident(name)),
    }
}

fn render_grant(privilege: &Privilege, dialect_name: &str, op: &DiffOp) -> Result<String> {
    let operations = render_privilege_ops(&privilege.operations, dialect_name, op)?;
    Ok(format!(
        "GRANT {} ON {} TO {}{}",
        operations,
        render_privilege_object(&privilege.on),
        render_ident(&privilege.grantee),
        if privilege.with_grant_option {
            " WITH GRANT OPTION"
        } else {
            ""
        }
    ))
}

fn render_revoke(privilege: &Privilege, dialect_name: &str, op: &DiffOp) -> Result<String> {
    let operations = render_privilege_ops(&privilege.operations, dialect_name, op)?;
    Ok(format!(
        "REVOKE {} ON {} FROM {}",
        operations,
        render_privilege_object(&privilege.on),
        render_ident(&privilege.grantee),
    ))
}

fn render_privilege_ops(
    operations: &[PrivilegeOp],
    dialect_name: &str,
    op: &DiffOp,
) -> Result<String> {
    if operations.is_empty() {
        return Err(unsupported_diff_op(
            dialect_name,
            op,
            "privilege operation list must not be empty",
        ));
    }

    Ok(operations
        .iter()
        .map(|operation| match operation {
            PrivilegeOp::Select => "SELECT",
            PrivilegeOp::Insert => "INSERT",
            PrivilegeOp::Update => "UPDATE",
            PrivilegeOp::Delete => "DELETE",
            PrivilegeOp::Truncate => "TRUNCATE",
            PrivilegeOp::References => "REFERENCES",
            PrivilegeOp::Trigger => "TRIGGER",
            PrivilegeOp::Usage => "USAGE",
            PrivilegeOp::Create => "CREATE",
            PrivilegeOp::Connect => "CONNECT",
            PrivilegeOp::Temporary => "TEMPORARY",
            PrivilegeOp::Execute => "EXECUTE",
            PrivilegeOp::All => "ALL",
        })
        .collect::<Vec<_>>()
        .join(", "))
}

fn render_privilege_object(object: &PrivilegeObject) -> String {
    match object {
        PrivilegeObject::Table(name) => format!("TABLE {}", render_qualified_name(name)),
        PrivilegeObject::View(name) => format!("TABLE {}", render_qualified_name(name)),
        PrivilegeObject::MaterializedView(name) => format!("TABLE {}", render_qualified_name(name)),
        PrivilegeObject::Sequence(name) => format!("SEQUENCE {}", render_qualified_name(name)),
        PrivilegeObject::Schema(name) => format!("SCHEMA {}", render_ident(name)),
        PrivilegeObject::Database(name) => format!("DATABASE {}", render_ident(name)),
        PrivilegeObject::Domain(name) => format!("DOMAIN {}", render_qualified_name(name)),
        PrivilegeObject::Type(name) => format!("TYPE {}", render_qualified_name(name)),
        PrivilegeObject::Function(name) => format!("FUNCTION {}", render_qualified_name(name)),
    }
}

fn render_create_policy(policy: &Policy) -> String {
    let mut sql = format!(
        "CREATE POLICY {} ON {} AS {}",
        render_ident(&policy.name),
        render_qualified_name(&policy.table),
        if policy.permissive {
            "PERMISSIVE"
        } else {
            "RESTRICTIVE"
        }
    );

    if let Some(command) = policy.command {
        write!(sql, " FOR {}", render_policy_command(command))
            .expect("writing to String should not fail");
    }

    if policy.roles.is_empty() {
        sql.push_str(" TO PUBLIC");
    } else {
        write!(sql, " TO {}", render_ident_list(&policy.roles))
            .expect("writing to String should not fail");
    }

    if let Some(using_expr) = &policy.using_expr {
        write!(sql, " USING ({})", render_expr(using_expr))
            .expect("writing to String should not fail");
    }

    if let Some(check_expr) = &policy.check_expr {
        write!(sql, " WITH CHECK ({})", render_expr(check_expr))
            .expect("writing to String should not fail");
    }

    sql
}

fn render_alter_table_options(
    table: &QualifiedName,
    options: &TableOptions,
    dialect_name: &str,
    op: &DiffOp,
) -> Result<String> {
    if options.extra.is_empty() {
        return Err(unsupported_diff_op(
            dialect_name,
            op,
            "table options must include at least one extra key",
        ));
    }

    let values = options
        .extra
        .iter()
        .map(|(key, value)| format!("{} = {}", key, render_value(value)))
        .collect::<Vec<_>>()
        .join(", ");

    Ok(format!(
        "ALTER TABLE {} SET ({values})",
        render_qualified_name(table)
    ))
}

fn render_alter_column_change(
    table: &QualifiedName,
    column: &Ident,
    change: &ColumnChange,
) -> String {
    let prefix = format!(
        "ALTER TABLE {} ALTER COLUMN {}",
        render_qualified_name(table),
        render_ident(column)
    );

    match change {
        ColumnChange::SetType(data_type) => {
            format!("{prefix} TYPE {}", render_data_type(data_type))
        }
        ColumnChange::SetNotNull(true) => format!("{prefix} SET NOT NULL"),
        ColumnChange::SetNotNull(false) => format!("{prefix} DROP NOT NULL"),
        ColumnChange::SetDefault(default_expr) => default_expr
            .as_ref()
            .map(|expr| format!("{prefix} SET DEFAULT {}", render_expr(expr)))
            .unwrap_or_else(|| format!("{prefix} DROP DEFAULT")),
        ColumnChange::SetIdentity(identity) => identity
            .as_ref()
            .map(|identity| format!("{prefix} ADD {}", render_identity(identity)))
            .unwrap_or_else(|| format!("{prefix} DROP IDENTITY IF EXISTS")),
        ColumnChange::SetGenerated(generated) => generated
            .as_ref()
            .map(|generated| {
                format!(
                    "{prefix} SET EXPRESSION AS ({}){}",
                    render_expr(&generated.expr),
                    if generated.stored { " STORED" } else { "" }
                )
            })
            .unwrap_or_else(|| format!("{prefix} DROP EXPRESSION")),
        ColumnChange::SetCollation(collation) => collation
            .as_ref()
            .map(|collation| {
                format!(
                    "{prefix} TYPE TEXT COLLATE {}",
                    render_ident(&Ident::unquoted(collation))
                )
            })
            .unwrap_or_else(|| format!("{prefix} TYPE TEXT")),
    }
}

fn render_add_index(index: &IndexDef, dialect_name: &str, op: &DiffOp) -> Result<String> {
    let name = index
        .name
        .as_ref()
        .ok_or_else(|| unsupported_diff_op(dialect_name, op, "index name is required"))?;

    let owner = render_index_owner(&index.owner);
    let columns = index
        .columns
        .iter()
        .map(|column| render_expr(&column.expr))
        .collect::<Vec<_>>()
        .join(", ");

    let mut sql = format!(
        "CREATE {}INDEX {}{} ON {}{} ({columns})",
        if index.unique { "UNIQUE " } else { "" },
        if index.concurrent {
            "CONCURRENTLY "
        } else {
            ""
        },
        render_ident(name),
        owner,
        index
            .method
            .as_ref()
            .map(|method| format!(" USING {method}"))
            .unwrap_or_default(),
    );

    if let Some(where_clause) = &index.where_clause {
        write!(sql, " WHERE {}", render_expr(where_clause))
            .expect("writing to String should not fail");
    }

    Ok(sql)
}

fn render_index_owner(owner: &IndexOwner) -> String {
    match owner {
        IndexOwner::Table(name) | IndexOwner::View(name) | IndexOwner::MaterializedView(name) => {
            render_qualified_name(name)
        }
    }
}

fn render_owner_scoped_name(owner: &IndexOwner, name: &Ident) -> String {
    let schema = match owner {
        IndexOwner::Table(owner_name)
        | IndexOwner::View(owner_name)
        | IndexOwner::MaterializedView(owner_name) => owner_name.schema.clone(),
    };

    let qualified = QualifiedName {
        schema,
        name: name.clone(),
    };

    render_qualified_name(&qualified)
}

fn render_partition_key(partition: &Partition) -> String {
    format!(
        "{} ({})",
        render_partition_strategy(partition.strategy.clone()),
        render_ident_list(&partition.columns)
    )
}

fn render_partition_bound(bound: &PartitionBound) -> String {
    match bound {
        PartitionBound::LessThan(values) => {
            format!("FOR VALUES LESS THAN ({})", render_expr_list(values))
        }
        PartitionBound::In(values) => format!("FOR VALUES IN ({})", render_expr_list(values)),
        PartitionBound::FromTo { from, to } => format!(
            "FOR VALUES FROM ({}) TO ({})",
            render_expr_list(from),
            render_expr_list(to)
        ),
        PartitionBound::MaxValue => "FOR VALUES IN (MAXVALUE)".to_string(),
    }
}

fn render_partition_strategy(strategy: PartitionStrategy) -> &'static str {
    match strategy {
        PartitionStrategy::Range => "RANGE",
        PartitionStrategy::List => "LIST",
        PartitionStrategy::Hash => "HASH",
        PartitionStrategy::Key => "KEY",
    }
}

fn render_column_position(position: &ColumnPosition) -> String {
    match position {
        ColumnPosition::First => "FIRST".to_string(),
        ColumnPosition::After(ident) => format!("AFTER {}", render_ident(ident)),
    }
}

fn render_data_type(data_type: &DataType) -> String {
    match data_type {
        DataType::Boolean => "boolean".to_string(),
        DataType::SmallInt => "smallint".to_string(),
        DataType::Integer => "integer".to_string(),
        DataType::BigInt => "bigint".to_string(),
        DataType::Real => "real".to_string(),
        DataType::DoublePrecision => "double precision".to_string(),
        DataType::Numeric { precision, scale } => match (precision, scale) {
            (Some(precision), Some(scale)) => format!("numeric({precision},{scale})"),
            (Some(precision), None) => format!("numeric({precision})"),
            _ => "numeric".to_string(),
        },
        DataType::Text => "text".to_string(),
        DataType::Varchar { length } => length
            .map(|length| format!("varchar({length})"))
            .unwrap_or_else(|| "varchar".to_string()),
        DataType::Char { length } => length
            .map(|length| format!("char({length})"))
            .unwrap_or_else(|| "char".to_string()),
        DataType::Blob => "bytea".to_string(),
        DataType::Date => "date".to_string(),
        DataType::Time { with_timezone } => {
            if *with_timezone {
                "time with time zone".to_string()
            } else {
                "time without time zone".to_string()
            }
        }
        DataType::Timestamp { with_timezone } => {
            if *with_timezone {
                "timestamp with time zone".to_string()
            } else {
                "timestamp without time zone".to_string()
            }
        }
        DataType::Json => "json".to_string(),
        DataType::Jsonb => "jsonb".to_string(),
        DataType::Uuid => "uuid".to_string(),
        DataType::Array(inner) => format!("{}[]", render_data_type(inner)),
        DataType::Custom(custom) => custom.clone(),
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
        Expr::Raw(raw) => raw.clone(),
        Expr::BinaryOp { left, op, right } => {
            format!(
                "({} {} {})",
                render_expr(left),
                render_binary_op(op),
                render_expr(right)
            )
        }
        Expr::UnaryOp { op, expr } => format!("({} {})", render_unary_op(op), render_expr(expr)),
        Expr::Comparison {
            left,
            op,
            right,
            quantifier,
        } => {
            if let Some(quantifier) = quantifier {
                format!(
                    "({} {} {} ({}))",
                    render_expr(left),
                    render_comparison_op(op),
                    render_set_quantifier(quantifier),
                    render_expr(right)
                )
            } else {
                format!(
                    "({} {} {})",
                    render_expr(left),
                    render_comparison_op(op),
                    render_expr(right)
                )
            }
        }
        Expr::And(left, right) => format!("({} AND {})", render_expr(left), render_expr(right)),
        Expr::Or(left, right) => format!("({} OR {})", render_expr(left), render_expr(right)),
        Expr::Not(expr) => format!("(NOT {})", render_expr(expr)),
        Expr::Is { expr, test } => format!("({} IS {})", render_expr(expr), render_is_test(test)),
        Expr::Between {
            expr,
            low,
            high,
            negated,
        } => format!(
            "({} {}BETWEEN {} AND {})",
            render_expr(expr),
            if *negated { "NOT " } else { "" },
            render_expr(low),
            render_expr(high)
        ),
        Expr::In {
            expr,
            list,
            negated,
        } => format!(
            "({} {}IN ({}))",
            render_expr(expr),
            if *negated { "NOT " } else { "" },
            render_expr_list(list)
        ),
        Expr::Paren(inner) => format!("({})", render_expr(inner)),
        Expr::Tuple(items) => format!("({})", render_expr_list(items)),
        Expr::Function {
            name,
            args,
            distinct,
            over,
        } => {
            let mut sql = format!(
                "{}({}{})",
                name,
                if *distinct { "DISTINCT " } else { "" },
                render_expr_list(args)
            );
            if let Some(window_spec) = over {
                let partition_by = if window_spec.partition_by.is_empty() {
                    String::new()
                } else {
                    format!(
                        "PARTITION BY {}",
                        render_expr_list(&window_spec.partition_by)
                    )
                };
                let order_by = if window_spec.order_by.is_empty() {
                    String::new()
                } else {
                    format!(
                        "{}ORDER BY {}",
                        if partition_by.is_empty() { "" } else { " " },
                        render_expr_list(&window_spec.order_by)
                    )
                };
                write!(sql, " OVER ({partition_by}{order_by})")
                    .expect("writing to String should not fail");
            }
            sql
        }
        Expr::Cast { expr, data_type } => {
            format!("({}::{} )", render_expr(expr), render_data_type(data_type)).replace(" )", ")")
        }
        Expr::Collate { expr, collation } => format!(
            "({} COLLATE {})",
            render_expr(expr),
            render_ident(&Ident::unquoted(collation))
        ),
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
        Expr::ArrayConstructor(items) => format!("ARRAY[{}]", render_expr_list(items)),
        Expr::Exists(subquery) => render_exists_subquery(subquery),
    }
}

fn render_exists_subquery(subquery: &SubQuery) -> String {
    format!("EXISTS ({})", subquery.sql)
}

fn render_literal(literal: &Literal) -> String {
    match literal {
        Literal::String(value) => quote_string(value),
        Literal::Integer(value) => value.to_string(),
        Literal::Float(value) => {
            if value.is_finite() {
                value.to_string()
            } else if value.is_nan() {
                quote_string("NaN")
            } else if value.is_sign_positive() {
                quote_string("Infinity")
            } else {
                quote_string("-Infinity")
            }
        }
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
        Value::String(value) => quote_string(value),
        Value::Integer(value) => value.to_string(),
        Value::Float(value) => value.to_string(),
        Value::Bool(value) => {
            if *value {
                "true".to_string()
            } else {
                "false".to_string()
            }
        }
        Value::Null => "NULL".to_string(),
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

fn render_deferrable(deferrable: Deferrable) -> &'static str {
    match deferrable {
        Deferrable::Deferrable {
            initially_deferred: true,
        } => "DEFERRABLE INITIALLY DEFERRED",
        Deferrable::Deferrable {
            initially_deferred: false,
        } => "DEFERRABLE INITIALLY IMMEDIATE",
        Deferrable::NotDeferrable => "NOT DEFERRABLE",
    }
}

fn render_sort_order(order: SortOrder) -> &'static str {
    match order {
        SortOrder::Asc => "ASC",
        SortOrder::Desc => "DESC",
    }
}

fn render_nulls_order(order: NullsOrder) -> &'static str {
    match order {
        NullsOrder::First => "FIRST",
        NullsOrder::Last => "LAST",
    }
}

fn render_identity(identity: &Identity) -> String {
    let mut sql = format!(
        "GENERATED {} AS IDENTITY",
        if identity.always {
            "ALWAYS"
        } else {
            "BY DEFAULT"
        }
    );

    let mut options = Vec::new();
    if let Some(start) = identity.start {
        options.push(format!("START WITH {start}"));
    }
    if let Some(increment) = identity.increment {
        options.push(format!("INCREMENT BY {increment}"));
    }
    if let Some(min_value) = identity.min_value {
        options.push(format!("MINVALUE {min_value}"));
    }
    if let Some(max_value) = identity.max_value {
        options.push(format!("MAXVALUE {max_value}"));
    }
    if let Some(cache) = identity.cache {
        options.push(format!("CACHE {cache}"));
    }
    options.push(if identity.cycle {
        "CYCLE".to_string()
    } else {
        "NO CYCLE".to_string()
    });

    if !options.is_empty() {
        write!(sql, " ({})", options.join(" ")).expect("writing to String should not fail");
    }

    sql
}

fn render_generated(generated: &GeneratedColumn) -> String {
    format!(
        "GENERATED ALWAYS AS ({}){}",
        render_expr(&generated.expr),
        if generated.stored { " STORED" } else { "" }
    )
}

fn render_binary_op(op: &BinaryOperator) -> &'static str {
    match op {
        BinaryOperator::Add => "+",
        BinaryOperator::Subtract => "-",
        BinaryOperator::Multiply => "*",
        BinaryOperator::Divide => "/",
        BinaryOperator::Modulo => "%",
        BinaryOperator::StringConcat => "||",
        BinaryOperator::BitwiseAnd => "&",
        BinaryOperator::BitwiseOr => "|",
        BinaryOperator::BitwiseXor => "#",
    }
}

fn render_unary_op(op: &UnaryOperator) -> &'static str {
    match op {
        UnaryOperator::Plus => "+",
        UnaryOperator::Minus => "-",
        UnaryOperator::Not => "NOT",
    }
}

fn render_comparison_op(op: &ComparisonOp) -> &'static str {
    match op {
        ComparisonOp::Equal => "=",
        ComparisonOp::NotEqual => "<>",
        ComparisonOp::GreaterThan => ">",
        ComparisonOp::GreaterThanOrEqual => ">=",
        ComparisonOp::LessThan => "<",
        ComparisonOp::LessThanOrEqual => "<=",
        ComparisonOp::Like => "LIKE",
        ComparisonOp::ILike => "ILIKE",
    }
}

fn render_set_quantifier(quantifier: &SetQuantifier) -> &'static str {
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

fn render_check_option(check_option: CheckOption) -> &'static str {
    match check_option {
        CheckOption::Local => "LOCAL",
        CheckOption::Cascaded => "CASCADED",
    }
}

fn render_view_security(security: ViewSecurity) -> &'static str {
    match security {
        ViewSecurity::Definer => "definer",
        ViewSecurity::Invoker => "invoker",
    }
}

fn render_trigger_timing(timing: TriggerTiming) -> &'static str {
    match timing {
        TriggerTiming::Before => "BEFORE",
        TriggerTiming::After => "AFTER",
        TriggerTiming::InsteadOf => "INSTEAD OF",
    }
}

fn render_trigger_event(event: &TriggerEvent) -> &'static str {
    match event {
        TriggerEvent::Insert => "INSERT",
        TriggerEvent::Update => "UPDATE",
        TriggerEvent::Delete => "DELETE",
        TriggerEvent::Truncate => "TRUNCATE",
    }
}

fn render_trigger_for_each(for_each: TriggerForEach) -> &'static str {
    match for_each {
        TriggerForEach::Row => "ROW",
        TriggerForEach::Statement => "STATEMENT",
    }
}

fn render_param_mode(mode: FunctionParamMode) -> &'static str {
    match mode {
        FunctionParamMode::In => "IN",
        FunctionParamMode::Out => "OUT",
        FunctionParamMode::InOut => "INOUT",
        FunctionParamMode::Variadic => "VARIADIC",
    }
}

fn render_volatility(volatility: Volatility) -> &'static str {
    match volatility {
        Volatility::Immutable => "IMMUTABLE",
        Volatility::Stable => "STABLE",
        Volatility::Volatile => "VOLATILE",
    }
}

fn render_function_security(security: FunctionSecurity) -> &'static str {
    match security {
        FunctionSecurity::Definer => "DEFINER",
        FunctionSecurity::Invoker => "INVOKER",
    }
}

fn render_policy_command(command: PolicyCommand) -> &'static str {
    match command {
        PolicyCommand::All => "ALL",
        PolicyCommand::Select => "SELECT",
        PolicyCommand::Insert => "INSERT",
        PolicyCommand::Update => "UPDATE",
        PolicyCommand::Delete => "DELETE",
    }
}

fn render_table_options_comment(options: &TableOptions) -> String {
    if options.extra.is_empty() {
        return String::new();
    }

    let payload = options
        .extra
        .iter()
        .map(|(key, value)| format!("{key}={}", render_value(value)))
        .collect::<Vec<_>>()
        .join(",");

    format!("/* table_options:{payload} */")
}

fn render_ident(ident: &Ident) -> String {
    let escaped = ident.value.replace('"', "\"\"");
    format!("\"{escaped}\"")
}

fn render_qualified_name(name: &QualifiedName) -> String {
    match &name.schema {
        Some(schema) => format!("{}.{}", render_ident(schema), render_ident(&name.name)),
        None => render_ident(&name.name),
    }
}

fn render_ident_list(items: &[Ident]) -> String {
    items
        .iter()
        .map(render_ident)
        .collect::<Vec<_>>()
        .join(", ")
}

fn render_expr_list(items: &[Expr]) -> String {
    items.iter().map(render_expr).collect::<Vec<_>>().join(", ")
}

fn render_dollar_quoted(body: &str) -> String {
    let mut tag = "$stateql$".to_string();
    while body.contains(&tag) {
        tag.insert(tag.len() - 1, '_');
    }
    format!("{tag}{body}{tag}")
}

fn quote_string(value: &str) -> String {
    format!("'{}'", value.replace('\'', "''"))
}

fn sql_statement(sql: String, transactional: bool) -> Statement {
    Statement::Sql {
        sql,
        transactional,
        context: None,
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
