use std::{collections::BTreeMap, io};

use pg_query::protobuf::{
    ConstrType, PartitionStrategy as PgPartitionStrategy, RawStmt, a_const, node::Node as NodeEnum,
};
use stateql_core::{
    AnnotationAttachment, AnnotationExtractor, AnnotationTarget, Column, DataType, Expr, Ident,
    Identity, Literal, ParseError, Partition, PartitionBound, PartitionElement, PartitionStrategy,
    QualifiedName, Result, SchemaObject, SourceLocation, Table, Value, attach_annotations,
};

use crate::{extra_keys, normalize};

type ConversionResult<T> = std::result::Result<T, io::Error>;

pub(crate) fn parse_schema(sql: &str) -> Result<Vec<SchemaObject>> {
    let (clean_sql, annotations) = AnnotationExtractor::extract(sql)?;
    let parse_result =
        pg_query::parse(&clean_sql).map_err(|source| ParseError::StatementConversion {
            statement_index: 0,
            source_sql: clean_sql.clone(),
            source_location: Some(SourceLocation {
                line: 1,
                column: None,
            }),
            source: Box::new(source),
        })?;

    let mut objects = Vec::with_capacity(parse_result.protobuf.stmts.len());
    let mut attachments = Vec::with_capacity(parse_result.protobuf.stmts.len());

    for (statement_index, statement) in parse_result.protobuf.stmts.iter().enumerate() {
        let metadata =
            statement_metadata(&clean_sql, &parse_result.protobuf.stmts, statement_index);
        let converted = convert_statement(statement, metadata.line).map_err(|source| {
            statement_conversion_error(
                statement_index,
                metadata.source_sql,
                metadata.source_location,
                source,
            )
        })?;

        objects.push(converted.object);
        attachments.push(converted.attachment);
    }

    attach_annotations(&mut objects, &annotations, &attachments)?;
    normalize::normalize_schema(&mut objects);
    Ok(objects)
}

struct ConvertedStatement {
    object: SchemaObject,
    attachment: AnnotationAttachment,
}

struct StatementMetadata {
    source_sql: String,
    source_location: Option<SourceLocation>,
    line: usize,
}

fn statement_conversion_error(
    statement_index: usize,
    source_sql: String,
    source_location: Option<SourceLocation>,
    source: io::Error,
) -> stateql_core::Error {
    ParseError::StatementConversion {
        statement_index,
        source_sql,
        source_location,
        source: Box::new(source),
    }
    .into()
}

fn statement_metadata(sql: &str, statements: &[RawStmt], index: usize) -> StatementMetadata {
    let statement = &statements[index];
    let start = statement_start_offset(statement);
    let end = statement_end_offset(sql, statements, index, start);
    let fragment = sql.get(start..end).unwrap_or(sql);
    let source_sql = fragment.trim();
    let source_sql = if source_sql.is_empty() {
        sql.trim().to_string()
    } else {
        source_sql.to_string()
    };
    let line_offset = start.saturating_add(leading_whitespace_len(fragment));
    let line = offset_to_line(sql, line_offset);

    StatementMetadata {
        source_sql,
        source_location: Some(SourceLocation { line, column: None }),
        line,
    }
}

fn leading_whitespace_len(fragment: &str) -> usize {
    fragment
        .char_indices()
        .find_map(|(index, ch)| (!ch.is_whitespace()).then_some(index))
        .unwrap_or(fragment.len())
}

fn statement_start_offset(statement: &RawStmt) -> usize {
    statement
        .stmt_location
        .try_into()
        .ok()
        .filter(|offset: &usize| *offset > 0)
        .unwrap_or(0)
}

fn statement_end_offset(sql: &str, statements: &[RawStmt], index: usize, start: usize) -> usize {
    if statement_starts_at_end(sql, start) {
        return sql.len();
    }

    if let Ok(stmt_len) = usize::try_from(statements[index].stmt_len)
        && stmt_len > 0
    {
        return start.saturating_add(stmt_len).min(sql.len());
    }

    if let Some(next_statement) = statements.get(index + 1)
        && let Ok(next_start) = usize::try_from(next_statement.stmt_location)
        && next_start > start
    {
        return next_start.min(sql.len());
    }

    sql.len()
}

fn statement_starts_at_end(sql: &str, start: usize) -> bool {
    start >= sql.len()
}

fn offset_to_line(sql: &str, offset: usize) -> usize {
    let end = offset.min(sql.len());
    let mut line = 1usize;

    for &byte in &sql.as_bytes()[..end] {
        if byte == b'\n' {
            line += 1;
        }
    }

    line
}

fn convert_statement(statement: &RawStmt, line: usize) -> ConversionResult<ConvertedStatement> {
    let node = statement
        .stmt
        .as_ref()
        .and_then(|stmt| stmt.node.as_ref())
        .ok_or_else(|| conversion_error("missing statement node"))?;

    match node {
        NodeEnum::CreateStmt(create_stmt) => convert_create_table(create_stmt, line),
        _ => Err(conversion_error(format!(
            "unsupported PostgreSQL statement kind: {}",
            statement_kind(node)
        ))),
    }
}

fn convert_create_table(
    create_stmt: &pg_query::protobuf::CreateStmt,
    line: usize,
) -> ConversionResult<ConvertedStatement> {
    let relation = create_stmt
        .relation
        .as_ref()
        .ok_or_else(|| conversion_error("CREATE TABLE is missing relation"))?;

    let mut table = Table::named(relation.relname.as_str());
    table.name = qualified_name_from_range_var(relation);

    for table_element in &create_stmt.table_elts {
        let element = table_element
            .node
            .as_ref()
            .ok_or_else(|| conversion_error("CREATE TABLE element is missing node payload"))?;

        match element {
            NodeEnum::ColumnDef(column_def) => {
                table.columns.push(convert_column(column_def)?);
            }
            NodeEnum::Constraint(_) => {
                return Err(conversion_error(
                    "unsupported CREATE TABLE element kind: Constraint",
                ));
            }
            _ => {
                return Err(conversion_error(format!(
                    "unsupported CREATE TABLE element kind: {}",
                    statement_kind(element)
                )));
            }
        }
    }

    if create_stmt.if_not_exists {
        table.options.extra.insert(
            extra_keys::TABLE_IF_NOT_EXISTS.to_string(),
            Value::Bool(true),
        );
    }
    if !create_stmt.access_method.is_empty() {
        table.options.extra.insert(
            extra_keys::TABLE_ACCESS_METHOD.to_string(),
            Value::String(create_stmt.access_method.clone()),
        );
    }
    if !create_stmt.tablespacename.is_empty() {
        table.options.extra.insert(
            extra_keys::TABLESPACE.to_string(),
            Value::String(create_stmt.tablespacename.clone()),
        );
    }

    apply_partition_metadata(create_stmt, &mut table)?;

    let attachment = AnnotationAttachment {
        line,
        target: AnnotationTarget::Table(table.name.clone()),
    };

    Ok(ConvertedStatement {
        object: SchemaObject::Table(table),
        attachment,
    })
}

fn convert_column(column_def: &pg_query::protobuf::ColumnDef) -> ConversionResult<Column> {
    let type_name = column_def.type_name.as_ref().ok_or_else(|| {
        conversion_error(format!(
            "column {} is missing type information",
            column_def.colname
        ))
    })?;

    let data_type = convert_data_type(type_name)?;
    let default = column_def
        .raw_default
        .as_ref()
        .map(|raw_default| {
            raw_default.deparse().map(Expr::Raw).map_err(|source| {
                conversion_error(format!(
                    "column {} default expression deparse failed: {source}",
                    column_def.colname
                ))
            })
        })
        .transpose()?;

    let mut extra = BTreeMap::new();
    if !column_def.identity.is_empty() {
        extra.insert(
            extra_keys::COLUMN_IDENTITY.to_string(),
            Value::String(column_def.identity.clone()),
        );
    }
    if !column_def.generated.is_empty() {
        extra.insert(
            extra_keys::COLUMN_GENERATED.to_string(),
            Value::String(column_def.generated.clone()),
        );
    }

    let identity = parse_identity(column_def)?;

    Ok(Column {
        name: Ident::unquoted(column_def.colname.as_str()),
        data_type,
        not_null: column_def.is_not_null,
        default,
        identity,
        generated: None,
        comment: None,
        collation: None,
        renamed_from: None,
        extra,
    })
}

fn parse_identity(
    column_def: &pg_query::protobuf::ColumnDef,
) -> ConversionResult<Option<Identity>> {
    for constraint_node in &column_def.constraints {
        let Some(NodeEnum::Constraint(constraint)) = constraint_node.node.as_ref() else {
            continue;
        };
        let Ok(constraint_type) = ConstrType::try_from(constraint.contype) else {
            continue;
        };
        if constraint_type == ConstrType::ConstrIdentity {
            return Ok(Some(identity_from_generated_when(
                constraint.generated_when.as_str(),
            )));
        }
    }

    Ok(None)
}

fn identity_from_generated_when(generated_when: &str) -> Identity {
    let normalized = generated_when.trim().to_ascii_lowercase();
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

fn apply_partition_metadata(
    create_stmt: &pg_query::protobuf::CreateStmt,
    table: &mut Table,
) -> ConversionResult<()> {
    if let Some(partition_spec) = &create_stmt.partspec {
        table.partition = Some(parse_partition_spec(partition_spec)?);
    }

    let parent = partition_parent(create_stmt);
    if let Some(parent) = parent {
        table.options.extra.insert(
            extra_keys::TABLE_PARTITION_PARENT_NAME.to_string(),
            Value::String(parent.relname.clone()),
        );
        if !parent.schemaname.is_empty() {
            table.options.extra.insert(
                extra_keys::TABLE_PARTITION_PARENT_SCHEMA.to_string(),
                Value::String(parent.schemaname.clone()),
            );
        }
    }

    if let Some(partition_bound) = &create_stmt.partbound {
        let strategy = partition_strategy_from_bound(partition_bound)
            .or_else(|| {
                table
                    .partition
                    .as_ref()
                    .map(|partition| partition.strategy.clone())
            })
            .unwrap_or(PartitionStrategy::Range);
        let bound = parse_partition_bound(partition_bound)?;
        table.partition = Some(Partition {
            strategy,
            columns: Vec::new(),
            partitions: vec![PartitionElement {
                name: table.name.name.clone(),
                bound,
                extra: BTreeMap::new(),
            }],
        });
    }

    Ok(())
}

fn parse_partition_spec(
    partition_spec: &pg_query::protobuf::PartitionSpec,
) -> ConversionResult<Partition> {
    let strategy = parse_partition_strategy(partition_spec.strategy)?;
    let mut columns = Vec::with_capacity(partition_spec.part_params.len());

    for part_param in &partition_spec.part_params {
        let Some(NodeEnum::PartitionElem(partition_elem)) = part_param.node.as_ref() else {
            return Err(conversion_error(
                "partition key is missing PartitionElem payload",
            ));
        };

        if !partition_elem.name.is_empty() {
            columns.push(Ident::unquoted(partition_elem.name.as_str()));
            continue;
        }

        let Some(expr) = &partition_elem.expr else {
            return Err(conversion_error(
                "partition key element has no name or expression",
            ));
        };
        columns.push(parse_partition_key_ident(expr.as_ref())?);
    }

    Ok(Partition {
        strategy,
        columns,
        partitions: Vec::new(),
    })
}

fn parse_partition_key_ident(node: &pg_query::protobuf::Node) -> ConversionResult<Ident> {
    match node.node.as_ref() {
        Some(NodeEnum::ColumnRef(column_ref)) => {
            let mut fields = column_ref.fields.iter().filter_map(node_string);
            let Some(first) = fields.next() else {
                return Err(conversion_error(
                    "partition key column reference has no identifier",
                ));
            };
            if fields.next().is_some() {
                return Err(conversion_error(
                    "partition key expression is unsupported in v1 parser",
                ));
            }
            Ok(Ident::unquoted(first))
        }
        Some(NodeEnum::String(value)) => Ok(Ident::unquoted(value.sval.as_str())),
        _ => Err(conversion_error(
            "partition key expression is unsupported in v1 parser",
        )),
    }
}

fn parse_partition_strategy(raw_strategy: i32) -> ConversionResult<PartitionStrategy> {
    let strategy = PgPartitionStrategy::try_from(raw_strategy).map_err(|_| {
        conversion_error(format!(
            "unsupported partition strategy code: {raw_strategy}"
        ))
    })?;

    match strategy {
        PgPartitionStrategy::List => Ok(PartitionStrategy::List),
        PgPartitionStrategy::Range => Ok(PartitionStrategy::Range),
        PgPartitionStrategy::Hash => Ok(PartitionStrategy::Hash),
        PgPartitionStrategy::Undefined => Err(conversion_error("partition strategy is undefined")),
    }
}

fn partition_strategy_from_bound(
    bound: &pg_query::protobuf::PartitionBoundSpec,
) -> Option<PartitionStrategy> {
    match bound.strategy.as_str() {
        "l" | "L" => Some(PartitionStrategy::List),
        "r" | "R" => Some(PartitionStrategy::Range),
        "h" | "H" => Some(PartitionStrategy::Hash),
        _ => None,
    }
}

fn parse_partition_bound(
    bound: &pg_query::protobuf::PartitionBoundSpec,
) -> ConversionResult<Option<PartitionBound>> {
    if bound.is_default {
        return Ok(None);
    }

    if !bound.listdatums.is_empty() {
        let values = parse_partition_expr_list(&bound.listdatums)?;
        if values.len() == 1 && is_maxvalue_expr(&values[0]) {
            return Ok(Some(PartitionBound::MaxValue));
        }
        return Ok(Some(PartitionBound::In(values)));
    }

    if !bound.lowerdatums.is_empty() || !bound.upperdatums.is_empty() {
        let from = parse_partition_expr_list(&bound.lowerdatums)?;
        let to = parse_partition_expr_list(&bound.upperdatums)?;
        if from.is_empty() {
            if to.len() == 1 && is_maxvalue_expr(&to[0]) {
                return Ok(Some(PartitionBound::MaxValue));
            }
            return Ok(Some(PartitionBound::LessThan(to)));
        }
        return Ok(Some(PartitionBound::FromTo { from, to }));
    }

    Ok(None)
}

fn parse_partition_expr_list(nodes: &[pg_query::protobuf::Node]) -> ConversionResult<Vec<Expr>> {
    nodes.iter().map(parse_partition_expr).collect()
}

fn parse_partition_expr(node: &pg_query::protobuf::Node) -> ConversionResult<Expr> {
    match node.node.as_ref() {
        Some(NodeEnum::AConst(constant)) => parse_partition_a_const(constant),
        Some(NodeEnum::ColumnRef(column_ref)) => {
            let token = column_ref
                .fields
                .iter()
                .find_map(node_string)
                .ok_or_else(|| {
                    conversion_error("partition bound column reference has no identifier")
                })?;
            if token.eq_ignore_ascii_case("maxvalue") {
                Ok(Expr::Raw("MAXVALUE".to_string()))
            } else if token.eq_ignore_ascii_case("minvalue") {
                Ok(Expr::Raw("MINVALUE".to_string()))
            } else {
                Ok(Expr::Ident(Ident::unquoted(token)))
            }
        }
        Some(NodeEnum::String(value)) => Ok(Expr::Literal(Literal::String(value.sval.clone()))),
        _ => Err(conversion_error("unsupported partition bound expression")),
    }
}

fn parse_partition_a_const(constant: &pg_query::protobuf::AConst) -> ConversionResult<Expr> {
    if constant.isnull {
        return Ok(Expr::Null);
    }

    match constant.val.as_ref() {
        Some(a_const::Val::Ival(value)) => {
            Ok(Expr::Literal(Literal::Integer(i64::from(value.ival))))
        }
        Some(a_const::Val::Fval(value)) => {
            let parsed = value.fval.parse::<f64>().map_err(|source| {
                conversion_error(format!(
                    "invalid float in partition bound expression: {source}"
                ))
            })?;
            Ok(Expr::Literal(Literal::Float(parsed)))
        }
        Some(a_const::Val::Boolval(value)) => Ok(Expr::Literal(Literal::Boolean(value.boolval))),
        Some(a_const::Val::Sval(value)) => Ok(Expr::Literal(Literal::String(value.sval.clone()))),
        Some(a_const::Val::Bsval(value)) => Ok(Expr::Raw(format!("B'{}'", value.bsval))),
        None => Err(conversion_error(
            "partition bound constant has no literal payload",
        )),
    }
}

fn is_maxvalue_expr(expr: &Expr) -> bool {
    matches!(expr, Expr::Raw(raw) if raw.eq_ignore_ascii_case("MAXVALUE"))
}

fn partition_parent(
    create_stmt: &pg_query::protobuf::CreateStmt,
) -> Option<&pg_query::protobuf::RangeVar> {
    create_stmt
        .inh_relations
        .iter()
        .filter_map(|node| node.node.as_ref())
        .find_map(|node| {
            if let NodeEnum::RangeVar(range_var) = node {
                Some(range_var)
            } else {
                None
            }
        })
}

fn convert_data_type(type_name: &pg_query::protobuf::TypeName) -> ConversionResult<DataType> {
    let names = type_name_parts(type_name);
    if names.is_empty() {
        return Err(conversion_error("type name has no identifiers"));
    }

    let base = names.last().cloned().unwrap_or_default();
    let base_lower = base.to_ascii_lowercase();
    let mut data_type = match base_lower.as_str() {
        "bool" | "boolean" => DataType::Boolean,
        "int2" | "smallint" => DataType::SmallInt,
        "int" | "int4" | "integer" => DataType::Integer,
        "int8" | "bigint" => DataType::BigInt,
        "float4" | "real" => DataType::Real,
        "float8" | "double" | "double precision" => DataType::DoublePrecision,
        "numeric" | "decimal" => DataType::Numeric {
            precision: None,
            scale: None,
        },
        "text" => DataType::Text,
        "varchar" | "character varying" => DataType::Varchar { length: None },
        "bpchar" | "char" | "character" => DataType::Char { length: None },
        "bytea" => DataType::Blob,
        "date" => DataType::Date,
        "time" => DataType::Time {
            with_timezone: false,
        },
        "timetz" => DataType::Time {
            with_timezone: true,
        },
        "timestamp" => DataType::Timestamp {
            with_timezone: false,
        },
        "timestamptz" => DataType::Timestamp {
            with_timezone: true,
        },
        "json" => DataType::Json,
        "jsonb" => DataType::Jsonb,
        "uuid" => DataType::Uuid,
        _ => DataType::Custom(names.join(".")),
    };

    if !type_name.array_bounds.is_empty() {
        data_type = DataType::Array(Box::new(data_type));
    }

    Ok(data_type)
}

fn type_name_parts(type_name: &pg_query::protobuf::TypeName) -> Vec<String> {
    type_name
        .names
        .iter()
        .filter_map(node_string)
        .map(ToOwned::to_owned)
        .collect()
}

fn qualified_name_from_range_var(range_var: &pg_query::protobuf::RangeVar) -> QualifiedName {
    QualifiedName {
        schema: (!range_var.schemaname.is_empty())
            .then(|| Ident::unquoted(range_var.schemaname.as_str())),
        name: Ident::unquoted(range_var.relname.as_str()),
    }
}

fn node_string(node: &pg_query::protobuf::Node) -> Option<&str> {
    match node.node.as_ref() {
        Some(NodeEnum::String(value)) => Some(value.sval.as_str()),
        _ => None,
    }
}

fn statement_kind(node: &NodeEnum) -> &'static str {
    match node {
        NodeEnum::CreateStmt(_) => "CreateStmt",
        NodeEnum::AlterTableStmt(_) => "AlterTableStmt",
        NodeEnum::IndexStmt(_) => "IndexStmt",
        NodeEnum::ViewStmt(_) => "ViewStmt",
        NodeEnum::CreateSeqStmt(_) => "CreateSeqStmt",
        NodeEnum::CreateSchemaStmt(_) => "CreateSchemaStmt",
        NodeEnum::CreateTrigStmt(_) => "CreateTrigStmt",
        NodeEnum::CreateFunctionStmt(_) => "CreateFunctionStmt",
        NodeEnum::CreateDomainStmt(_) => "CreateDomainStmt",
        NodeEnum::CreateEnumStmt(_) => "CreateEnumStmt",
        NodeEnum::CreateExtensionStmt(_) => "CreateExtensionStmt",
        NodeEnum::CreatePolicyStmt(_) => "CreatePolicyStmt",
        NodeEnum::CommentStmt(_) => "CommentStmt",
        _ => "Other",
    }
}

fn conversion_error(message: impl Into<String>) -> io::Error {
    io::Error::other(message.into())
}
