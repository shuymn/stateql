use std::{collections::BTreeMap, io};

use pg_query::protobuf::{RawStmt, node::Node as NodeEnum};
use stateql_core::{
    AnnotationAttachment, AnnotationExtractor, AnnotationTarget, Column, DataType, Expr, Ident,
    ParseError, QualifiedName, Result, SchemaObject, SourceLocation, Table, Value,
    attach_annotations,
};

use crate::extra_keys;

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

    Ok(Column {
        name: Ident::unquoted(column_def.colname.as_str()),
        data_type,
        not_null: column_def.is_not_null,
        default,
        identity: None,
        generated: None,
        comment: None,
        collation: None,
        renamed_from: None,
        extra,
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
