use std::io;

use sqlparser::{
    ast::{CreateView, CreateViewSecurity, ObjectName, Statement},
    dialect::MySqlDialect,
    parser::Parser,
};
use stateql_core::{
    AnnotationAttachment, AnnotationExtractor, AnnotationTarget, Ident, ParseError, QualifiedName,
    Result, SchemaObject, SourceLocation, Table, Value, View, ViewSecurity, attach_annotations,
};

use crate::extra_keys;

type ConversionResult<T> = std::result::Result<T, io::Error>;

pub(crate) fn parse_schema(sql: &str) -> Result<Vec<SchemaObject>> {
    let (clean_sql, annotations) = AnnotationExtractor::extract(sql)?;
    let ast = Parser::parse_sql(&MySqlDialect {}, &clean_sql).map_err(|source| {
        ParseError::StatementConversion {
            statement_index: 0,
            source_sql: clean_sql.clone(),
            source_location: Some(SourceLocation {
                line: 1,
                column: None,
            }),
            source: Box::new(source),
        }
    })?;

    let metadata = statement_metadata(&clean_sql);
    let mut objects = Vec::with_capacity(ast.len());
    let mut attachments = Vec::with_capacity(ast.len());

    for (statement_index, statement) in ast.iter().enumerate() {
        let metadata = metadata
            .get(statement_index)
            .cloned()
            .unwrap_or_else(|| fallback_metadata(statement));
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

#[derive(Clone)]
struct StatementMetadata {
    source_sql: String,
    source_location: Option<SourceLocation>,
    line: usize,
}

struct ConvertedStatement {
    object: SchemaObject,
    attachment: AnnotationAttachment,
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

fn statement_metadata(sql: &str) -> Vec<StatementMetadata> {
    split_statement_spans(sql)
        .into_iter()
        .filter_map(|(start, end)| {
            let fragment = sql.get(start..end).unwrap_or(sql);
            let source_sql = fragment.trim();
            if source_sql.is_empty() {
                return None;
            }
            let line_offset = start.saturating_add(leading_whitespace_len(fragment));
            let line = offset_to_line(sql, line_offset);
            Some(StatementMetadata {
                source_sql: source_sql.to_string(),
                source_location: Some(SourceLocation { line, column: None }),
                line,
            })
        })
        .collect()
}

fn split_statement_spans(sql: &str) -> Vec<(usize, usize)> {
    let bytes = sql.as_bytes();
    let mut spans = Vec::new();
    let mut start = 0usize;
    let mut index = 0usize;
    let mut in_single_quote = false;
    let mut in_double_quote = false;
    let mut in_backtick_quote = false;
    let mut in_bracket_ident = false;
    let mut in_line_comment = false;
    let mut in_block_comment = false;

    while index < bytes.len() {
        let byte = bytes[index];

        if in_line_comment {
            if byte == b'\n' {
                in_line_comment = false;
            }
            index += 1;
            continue;
        }

        if in_block_comment {
            if byte == b'*' && bytes.get(index + 1) == Some(&b'/') {
                in_block_comment = false;
                index += 2;
                continue;
            }
            index += 1;
            continue;
        }

        if in_single_quote {
            if byte == b'\\' {
                index = (index + 2).min(bytes.len());
                continue;
            }
            if byte == b'\'' {
                if bytes.get(index + 1) == Some(&b'\'') {
                    index += 2;
                    continue;
                }
                in_single_quote = false;
            }
            index += 1;
            continue;
        }

        if in_double_quote {
            if byte == b'"' {
                if bytes.get(index + 1) == Some(&b'"') {
                    index += 2;
                    continue;
                }
                in_double_quote = false;
            }
            index += 1;
            continue;
        }

        if in_backtick_quote {
            if byte == b'`' {
                if bytes.get(index + 1) == Some(&b'`') {
                    index += 2;
                    continue;
                }
                in_backtick_quote = false;
            }
            index += 1;
            continue;
        }

        if in_bracket_ident {
            if byte == b']' {
                if bytes.get(index + 1) == Some(&b']') {
                    index += 2;
                    continue;
                }
                in_bracket_ident = false;
            }
            index += 1;
            continue;
        }

        if byte == b'-' && bytes.get(index + 1) == Some(&b'-') {
            in_line_comment = true;
            index += 2;
            continue;
        }

        if byte == b'/' && bytes.get(index + 1) == Some(&b'*') {
            in_block_comment = true;
            index += 2;
            continue;
        }

        match byte {
            b'\'' => {
                in_single_quote = true;
            }
            b'"' => {
                in_double_quote = true;
            }
            b'`' => {
                in_backtick_quote = true;
            }
            b'[' => {
                in_bracket_ident = true;
            }
            b';' => {
                spans.push((start, index + 1));
                start = index + 1;
            }
            _ => {}
        }

        index += 1;
    }

    if start < bytes.len() {
        spans.push((start, bytes.len()));
    }

    if spans.is_empty() && !sql.trim().is_empty() {
        spans.push((0, bytes.len()));
    }

    spans
}

fn fallback_metadata(statement: &Statement) -> StatementMetadata {
    StatementMetadata {
        source_sql: statement.to_string(),
        source_location: Some(SourceLocation {
            line: 1,
            column: None,
        }),
        line: 1,
    }
}

fn leading_whitespace_len(fragment: &str) -> usize {
    fragment
        .char_indices()
        .find_map(|(index, ch)| (!ch.is_whitespace()).then_some(index))
        .unwrap_or(fragment.len())
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

fn convert_statement(statement: &Statement, line: usize) -> ConversionResult<ConvertedStatement> {
    let statement_sql = statement.to_string();
    match statement {
        Statement::CreateTable(_) => convert_create_table_statement(&statement_sql, line),
        Statement::CreateView(create_view) => convert_create_view_statement(create_view, line),
        _ => Err(conversion_error(format!(
            "unsupported mysql statement kind: {}",
            statement_kind(&statement_sql)
        ))),
    }
}

fn convert_create_table_statement(
    statement_sql: &str,
    line: usize,
) -> ConversionResult<ConvertedStatement> {
    let table_name = parse_create_table_name(statement_sql)
        .ok_or_else(|| conversion_error("failed to parse CREATE TABLE name"))?;
    let mut table = Table::named(table_name.name.value.as_str());
    table.name = table_name.clone();
    apply_preconversion_hints(statement_sql, &mut table);

    Ok(ConvertedStatement {
        object: SchemaObject::Table(table),
        attachment: AnnotationAttachment {
            line,
            target: AnnotationTarget::Table(table_name),
        },
    })
}

fn convert_create_view_statement(
    create_view: &CreateView,
    line: usize,
) -> ConversionResult<ConvertedStatement> {
    if create_view.materialized || create_view.secure || create_view.temporary {
        return Err(conversion_error(
            "unsupported CREATE VIEW variant: materialized/secure/temporary",
        ));
    }
    if create_view.to.is_some() || create_view.with_no_schema_binding {
        return Err(conversion_error(
            "unsupported CREATE VIEW clause: TO or WITH NO SCHEMA BINDING",
        ));
    }
    if create_view
        .params
        .as_ref()
        .is_some_and(|params| params.algorithm.is_some() || params.definer.is_some())
    {
        return Err(conversion_error(
            "unsupported CREATE VIEW parameters: ALGORITHM/DEFINER",
        ));
    }

    let name = parse_object_name(&create_view.name)?;
    let mut view = View::new(name.clone(), create_view.query.to_string());
    view.columns = create_view
        .columns
        .iter()
        .map(|column| parse_sqlparser_ident(&column.name))
        .collect();
    view.security = create_view
        .params
        .as_ref()
        .and_then(|params| params.security.as_ref())
        .map(parse_view_security);

    Ok(ConvertedStatement {
        object: SchemaObject::View(view),
        attachment: AnnotationAttachment {
            line,
            target: AnnotationTarget::View(name),
        },
    })
}

fn apply_preconversion_hints(statement_sql: &str, table: &mut Table) {
    table.options.extra.insert(
        extra_keys::TABLE_SOURCE_SQL.to_string(),
        Value::String(statement_sql.trim().to_string()),
    );

    let normalized = statement_sql.to_ascii_uppercase();

    if normalized.contains("CHANGE COLUMN") {
        table.options.extra.insert(
            extra_keys::TABLE_HAS_CHANGE_COLUMN.to_string(),
            Value::Bool(true),
        );
    }

    if normalized.contains(" AFTER ") {
        table.options.extra.insert(
            extra_keys::TABLE_HAS_AFTER_CLAUSE.to_string(),
            Value::Bool(true),
        );
    }

    if normalized.contains("AUTO_INCREMENT") {
        table.options.extra.insert(
            extra_keys::TABLE_HAS_AUTO_INCREMENT.to_string(),
            Value::Bool(true),
        );
    }

    if normalized.contains("PARTITION BY") {
        table.options.extra.insert(
            extra_keys::TABLE_HAS_PARTITIONING.to_string(),
            Value::Bool(true),
        );
        if let Some(partition_clause) = extract_partition_clause(statement_sql) {
            table.options.extra.insert(
                extra_keys::TABLE_PARTITION_SQL.to_string(),
                Value::String(partition_clause),
            );
        }
    }
}

fn extract_partition_clause(statement_sql: &str) -> Option<String> {
    let marker = "PARTITION BY";
    let index = statement_sql
        .to_ascii_uppercase()
        .find(marker)
        .or_else(|| statement_sql.to_ascii_uppercase().find(" PARTITION "))?;
    let clause = statement_sql
        .get(index..)?
        .trim()
        .trim_end_matches(';')
        .trim();
    if clause.is_empty() {
        None
    } else {
        Some(clause.to_string())
    }
}

fn parse_create_table_name(statement_sql: &str) -> Option<QualifiedName> {
    let tokens: Vec<&str> = statement_sql.split_whitespace().collect();
    if !tokens
        .first()
        .is_some_and(|token| eq_keyword(token, "CREATE"))
    {
        return None;
    }

    let mut cursor = 1usize;
    if tokens
        .get(cursor)
        .is_some_and(|token| eq_keyword(token, "TEMP") || eq_keyword(token, "TEMPORARY"))
    {
        cursor += 1;
    }

    if !tokens
        .get(cursor)
        .is_some_and(|token| eq_keyword(token, "TABLE"))
    {
        return None;
    }
    cursor += 1;

    if tokens
        .get(cursor)
        .is_some_and(|token| eq_keyword(token, "IF"))
        && tokens
            .get(cursor + 1)
            .is_some_and(|token| eq_keyword(token, "NOT"))
        && tokens
            .get(cursor + 2)
            .is_some_and(|token| eq_keyword(token, "EXISTS"))
    {
        cursor += 3;
    }

    parse_qualified_name_token(tokens.get(cursor)?)
}

fn parse_object_name(name: &ObjectName) -> ConversionResult<QualifiedName> {
    if name.0.is_empty() || name.0.len() > 2 {
        return Err(conversion_error(format!(
            "unsupported qualified name in CREATE VIEW: {}",
            name
        )));
    }

    let identifiers = name
        .0
        .iter()
        .map(|part| {
            part.as_ident().ok_or_else(|| {
                conversion_error(format!(
                    "unsupported object name part in CREATE VIEW: {}",
                    part
                ))
            })
        })
        .collect::<ConversionResult<Vec<_>>>()?;

    if identifiers.len() == 1 {
        return Ok(QualifiedName {
            schema: None,
            name: parse_sqlparser_ident(identifiers[0]),
        });
    }

    Ok(QualifiedName {
        schema: Some(parse_sqlparser_ident(identifiers[0])),
        name: parse_sqlparser_ident(identifiers[1]),
    })
}

fn parse_sqlparser_ident(ident: &sqlparser::ast::Ident) -> Ident {
    if ident.quote_style.is_some() {
        Ident::quoted(ident.value.clone())
    } else {
        Ident::unquoted(ident.value.clone())
    }
}

fn parse_view_security(security: &CreateViewSecurity) -> ViewSecurity {
    match security {
        CreateViewSecurity::Definer => ViewSecurity::Definer,
        CreateViewSecurity::Invoker => ViewSecurity::Invoker,
    }
}

fn parse_qualified_name_token(raw: &str) -> Option<QualifiedName> {
    let token = trim_identifier_token(raw);
    if token.is_empty() {
        return None;
    }

    let parts = split_qualified_name(token);
    if parts.is_empty() || parts.len() > 2 {
        return None;
    }

    if parts.len() == 1 {
        let name = parse_ident_token(&parts[0])?;
        return Some(QualifiedName { schema: None, name });
    }

    let schema = parse_ident_token(&parts[0])?;
    let name = parse_ident_token(&parts[1])?;
    Some(QualifiedName {
        schema: Some(schema),
        name,
    })
}

fn split_qualified_name(token: &str) -> Vec<String> {
    let mut parts = Vec::new();
    let mut current = String::new();
    let mut in_double_quote = false;
    let mut in_backtick_quote = false;
    let mut in_bracket_ident = false;

    for ch in token.chars() {
        if in_double_quote {
            current.push(ch);
            if ch == '"' {
                in_double_quote = false;
            }
            continue;
        }

        if in_backtick_quote {
            current.push(ch);
            if ch == '`' {
                in_backtick_quote = false;
            }
            continue;
        }

        if in_bracket_ident {
            current.push(ch);
            if ch == ']' {
                in_bracket_ident = false;
            }
            continue;
        }

        match ch {
            '.' => {
                parts.push(current.trim().to_string());
                current.clear();
            }
            '"' => {
                in_double_quote = true;
                current.push(ch);
            }
            '`' => {
                in_backtick_quote = true;
                current.push(ch);
            }
            '[' => {
                in_bracket_ident = true;
                current.push(ch);
            }
            _ => {
                current.push(ch);
            }
        }
    }

    if !current.is_empty() {
        parts.push(current.trim().to_string());
    }

    parts
}

fn parse_ident_token(raw: &str) -> Option<Ident> {
    let token = trim_identifier_token(raw);
    if token.is_empty() {
        return None;
    }

    if let Some(inner) = token
        .strip_prefix('`')
        .and_then(|rest| rest.strip_suffix('`'))
    {
        return Some(Ident::quoted(inner.replace("``", "`")));
    }

    if let Some(inner) = token
        .strip_prefix('"')
        .and_then(|rest| rest.strip_suffix('"'))
    {
        return Some(Ident::quoted(inner.replace("\"\"", "\"")));
    }

    if let Some(inner) = token
        .strip_prefix('[')
        .and_then(|rest| rest.strip_suffix(']'))
    {
        return Some(Ident::quoted(inner.replace("]]", "]")));
    }

    Some(Ident::unquoted(token))
}

fn trim_identifier_token(raw: &str) -> &str {
    raw.trim()
        .trim_start_matches('(')
        .trim_end_matches(';')
        .trim_end_matches(',')
        .trim_end_matches(')')
}

fn statement_kind(statement_sql: &str) -> String {
    let mut words = statement_sql.split_whitespace();
    let first = words.next().unwrap_or("unknown");
    if let Some(second) = words.next() {
        format!("{first} {second}")
    } else {
        first.to_string()
    }
}

fn eq_keyword(token: &str, keyword: &str) -> bool {
    trim_identifier_token(token).eq_ignore_ascii_case(keyword)
}

fn conversion_error(message: impl Into<String>) -> io::Error {
    io::Error::other(message.into())
}
