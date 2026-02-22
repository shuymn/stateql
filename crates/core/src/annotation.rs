use crate::{DiffError, Ident, QualifiedName, Result, SchemaObject};

const RENAMED_KEYWORD: &str = "@renamed";
const RENAME_ALIAS_KEYWORD: &str = "@rename";

/// Extracts rename annotations from SQL comments before parser invocation.
pub struct AnnotationExtractor;

/// A rename annotation captured from SQL comments.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RenameAnnotation {
    /// 1-based source line number in the original SQL.
    pub line: usize,
    /// Source identifier specified by `from=...`.
    pub from: Ident,
    /// `true` when extracted from deprecated `@rename` alias.
    pub deprecated_alias: bool,
}

/// Source-line attachment point for a parsed schema object.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AnnotationAttachment {
    /// 1-based source line number in the original SQL.
    pub line: usize,
    /// Parsed schema object key expected at this line.
    pub target: AnnotationTarget,
}

/// Target key used to attach a rename annotation to an IR object.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AnnotationTarget {
    Table(QualifiedName),
    View(QualifiedName),
    MaterializedView(QualifiedName),
    TableColumn { table: QualifiedName, column: Ident },
    MaterializedViewColumn { view: QualifiedName, column: Ident },
}

impl AnnotationExtractor {
    /// Extracts `@renamed` / `@rename` annotations from line comments.
    ///
    /// The returned SQL preserves original line boundaries.
    pub fn extract(sql: &str) -> Result<(String, Vec<RenameAnnotation>)> {
        let mut cleaned_sql = String::with_capacity(sql.len());
        let mut annotations = Vec::new();

        for (line_index, raw_line) in sql.split_inclusive('\n').enumerate() {
            let (line, line_ending) = split_line_ending(raw_line);
            let mut cleaned_line = line.to_string();

            if let Some(comment_start) = find_line_comment_start(line) {
                let comment = &line[comment_start + 2..];
                if let Some(parsed) = parse_annotation(comment) {
                    annotations.push(RenameAnnotation {
                        line: line_index + 1,
                        from: parsed.from,
                        deprecated_alias: parsed.deprecated_alias,
                    });

                    cleaned_line = String::with_capacity(line.len() - (parsed.end - parsed.start));
                    cleaned_line.push_str(&line[..comment_start + 2]);
                    cleaned_line.push_str(&comment[..parsed.start]);
                    cleaned_line.push_str(&comment[parsed.end..]);
                }
            }

            cleaned_sql.push_str(&cleaned_line);
            cleaned_sql.push_str(line_ending);
        }

        Ok((cleaned_sql, annotations))
    }
}

/// Attaches rename annotations to parsed schema objects.
///
/// The function is fail-fast: if any annotation cannot be attached, no objects
/// are mutated and an error is returned.
pub fn attach_annotations(
    objects: &mut [SchemaObject],
    annotations: &[RenameAnnotation],
    attachments: &[AnnotationAttachment],
) -> Result<()> {
    let mut ops = Vec::with_capacity(annotations.len());

    for annotation in annotations {
        let attachment = find_attachment_for_line(attachments, annotation)?;
        let op = resolve_attachment(objects, &attachment.target, annotation)?;
        ops.push(op);
    }

    for op in ops {
        apply_attachment(objects, op);
    }

    Ok(())
}

struct ParsedAnnotation {
    start: usize,
    end: usize,
    from: Ident,
    deprecated_alias: bool,
}

fn split_line_ending(raw_line: &str) -> (&str, &str) {
    if let Some(line) = raw_line.strip_suffix('\n') {
        (line, "\n")
    } else {
        (raw_line, "")
    }
}

fn find_line_comment_start(line: &str) -> Option<usize> {
    let bytes = line.as_bytes();
    let mut index = 0;
    let mut in_single_quote = false;
    let mut in_double_quote = false;

    while index < bytes.len() {
        if in_single_quote {
            if bytes[index] == b'\'' {
                if index + 1 < bytes.len() && bytes[index + 1] == b'\'' {
                    index += 2;
                    continue;
                }
                in_single_quote = false;
            }
            index += 1;
            continue;
        }

        if in_double_quote {
            if bytes[index] == b'"' {
                if index + 1 < bytes.len() && bytes[index + 1] == b'"' {
                    index += 2;
                    continue;
                }
                in_double_quote = false;
            }
            index += 1;
            continue;
        }

        match bytes[index] {
            b'\'' => {
                in_single_quote = true;
                index += 1;
            }
            b'"' => {
                in_double_quote = true;
                index += 1;
            }
            b'-' if index + 1 < bytes.len() && bytes[index + 1] == b'-' => {
                return Some(index);
            }
            _ => {
                index += 1;
            }
        }
    }

    None
}

fn parse_annotation(comment: &str) -> Option<ParsedAnnotation> {
    let mut search_from = 0;
    while let Some(relative_at) = comment[search_from..].find('@') {
        let at = search_from + relative_at;
        if let Some(parsed) = parse_annotation_at(comment, at) {
            return Some(parsed);
        }
        search_from = at + 1;
    }

    None
}

fn parse_annotation_at(comment: &str, start: usize) -> Option<ParsedAnnotation> {
    let remaining = &comment[start..];
    let (keyword_len, deprecated_alias) = if remaining.starts_with(RENAMED_KEYWORD) {
        (RENAMED_KEYWORD.len(), false)
    } else if remaining.starts_with(RENAME_ALIAS_KEYWORD) {
        (RENAME_ALIAS_KEYWORD.len(), true)
    } else {
        return None;
    };

    let mut cursor = start + keyword_len;
    if let Some(ch) = comment[cursor..].chars().next()
        && !ch.is_ascii_whitespace()
    {
        return None;
    }

    cursor = skip_ascii_whitespace(comment, cursor);
    if !comment[cursor..].starts_with("from") {
        return None;
    }
    cursor += "from".len();

    cursor = skip_ascii_whitespace(comment, cursor);
    if !comment[cursor..].starts_with('=') {
        return None;
    }
    cursor += 1;

    cursor = skip_ascii_whitespace(comment, cursor);
    let (from, end) = parse_ident(comment, cursor)?;

    Some(ParsedAnnotation {
        start,
        end,
        from,
        deprecated_alias,
    })
}

fn skip_ascii_whitespace(s: &str, mut index: usize) -> usize {
    let bytes = s.as_bytes();
    while index < bytes.len() && bytes[index].is_ascii_whitespace() {
        index += 1;
    }
    index
}

fn parse_ident(input: &str, start: usize) -> Option<(Ident, usize)> {
    let bytes = input.as_bytes();
    if start >= bytes.len() {
        return None;
    }

    if bytes[start] == b'"' {
        let mut index = start + 1;
        let mut value = String::new();

        while index < bytes.len() {
            if bytes[index] == b'"' {
                if index + 1 < bytes.len() && bytes[index + 1] == b'"' {
                    value.push('"');
                    index += 2;
                    continue;
                }

                return Some((Ident::quoted(value), index + 1));
            }

            value.push(bytes[index] as char);
            index += 1;
        }

        return None;
    }

    let mut index = start;
    while index < bytes.len() && !bytes[index].is_ascii_whitespace() {
        index += 1;
    }

    if index == start {
        return None;
    }

    Some((Ident::unquoted(&input[start..index]), index))
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum AttachmentOperation {
    Table {
        object_index: usize,
        from: Ident,
    },
    View {
        object_index: usize,
        from: Ident,
    },
    MaterializedView {
        object_index: usize,
        from: Ident,
    },
    TableColumn {
        object_index: usize,
        column_index: usize,
        from: Ident,
    },
    MaterializedViewColumn {
        object_index: usize,
        column_index: usize,
        from: Ident,
    },
}

fn find_attachment_for_line<'a>(
    attachments: &'a [AnnotationAttachment],
    annotation: &RenameAnnotation,
) -> Result<&'a AnnotationAttachment> {
    let mut matches = attachments
        .iter()
        .filter(|attachment| attachment.line == annotation.line);
    let Some(first_match) = matches.next() else {
        return Err(orphan_annotation_error(annotation));
    };

    if matches.next().is_some() {
        return Err(orphan_annotation_error(annotation));
    }

    Ok(first_match)
}

fn resolve_attachment(
    objects: &[SchemaObject],
    target: &AnnotationTarget,
    annotation: &RenameAnnotation,
) -> Result<AttachmentOperation> {
    match target {
        AnnotationTarget::Table(name) => {
            let mut matches =
                objects
                    .iter()
                    .enumerate()
                    .filter_map(|(index, object)| match object {
                        SchemaObject::Table(table) if qualified_name_matches(&table.name, name) => {
                            Some(index)
                        }
                        _ => None,
                    });
            let Some(object_index) = matches.next() else {
                return Err(orphan_annotation_error(annotation));
            };
            if matches.next().is_some() {
                return Err(orphan_annotation_error(annotation));
            }
            Ok(AttachmentOperation::Table {
                object_index,
                from: annotation.from.clone(),
            })
        }
        AnnotationTarget::View(name) => {
            let mut matches =
                objects
                    .iter()
                    .enumerate()
                    .filter_map(|(index, object)| match object {
                        SchemaObject::View(view) if qualified_name_matches(&view.name, name) => {
                            Some(index)
                        }
                        _ => None,
                    });
            let Some(object_index) = matches.next() else {
                return Err(orphan_annotation_error(annotation));
            };
            if matches.next().is_some() {
                return Err(orphan_annotation_error(annotation));
            }
            Ok(AttachmentOperation::View {
                object_index,
                from: annotation.from.clone(),
            })
        }
        AnnotationTarget::MaterializedView(name) => {
            let mut matches =
                objects
                    .iter()
                    .enumerate()
                    .filter_map(|(index, object)| match object {
                        SchemaObject::MaterializedView(view)
                            if qualified_name_matches(&view.name, name) =>
                        {
                            Some(index)
                        }
                        _ => None,
                    });
            let Some(object_index) = matches.next() else {
                return Err(orphan_annotation_error(annotation));
            };
            if matches.next().is_some() {
                return Err(orphan_annotation_error(annotation));
            }
            Ok(AttachmentOperation::MaterializedView {
                object_index,
                from: annotation.from.clone(),
            })
        }
        AnnotationTarget::TableColumn { table, column } => {
            let Some((object_index, column_index)) =
                find_table_column_index(objects, table, column)
            else {
                return Err(orphan_annotation_error(annotation));
            };
            Ok(AttachmentOperation::TableColumn {
                object_index,
                column_index,
                from: annotation.from.clone(),
            })
        }
        AnnotationTarget::MaterializedViewColumn { view, column } => {
            let Some((object_index, column_index)) =
                find_materialized_view_column_index(objects, view, column)
            else {
                return Err(orphan_annotation_error(annotation));
            };
            Ok(AttachmentOperation::MaterializedViewColumn {
                object_index,
                column_index,
                from: annotation.from.clone(),
            })
        }
    }
}

fn find_table_column_index(
    objects: &[SchemaObject],
    table_name: &QualifiedName,
    column_name: &Ident,
) -> Option<(usize, usize)> {
    let mut object_matches =
        objects
            .iter()
            .enumerate()
            .filter_map(|(index, object)| match object {
                SchemaObject::Table(table) if qualified_name_matches(&table.name, table_name) => {
                    Some((index, table))
                }
                _ => None,
            });
    let (object_index, table) = object_matches.next()?;
    if object_matches.next().is_some() {
        return None;
    }

    let mut column_matches =
        table
            .columns
            .iter()
            .enumerate()
            .filter_map(|(column_index, column)| {
                ident_matches(&column.name, column_name).then_some(column_index)
            });
    let column_index = column_matches.next()?;
    if column_matches.next().is_some() {
        return None;
    }

    Some((object_index, column_index))
}

fn find_materialized_view_column_index(
    objects: &[SchemaObject],
    view_name: &QualifiedName,
    column_name: &Ident,
) -> Option<(usize, usize)> {
    let mut object_matches =
        objects
            .iter()
            .enumerate()
            .filter_map(|(index, object)| match object {
                SchemaObject::MaterializedView(view)
                    if qualified_name_matches(&view.name, view_name) =>
                {
                    Some((index, view))
                }
                _ => None,
            });
    let (object_index, view) = object_matches.next()?;
    if object_matches.next().is_some() {
        return None;
    }

    let mut column_matches =
        view.columns
            .iter()
            .enumerate()
            .filter_map(|(column_index, column)| {
                ident_matches(&column.name, column_name).then_some(column_index)
            });
    let column_index = column_matches.next()?;
    if column_matches.next().is_some() {
        return None;
    }

    Some((object_index, column_index))
}

fn apply_attachment(objects: &mut [SchemaObject], op: AttachmentOperation) {
    match op {
        AttachmentOperation::Table { object_index, from } => {
            if let SchemaObject::Table(table) = &mut objects[object_index] {
                table.renamed_from = Some(from);
            }
        }
        AttachmentOperation::View { object_index, from } => {
            if let SchemaObject::View(view) = &mut objects[object_index] {
                view.renamed_from = Some(from);
            }
        }
        AttachmentOperation::MaterializedView { object_index, from } => {
            if let SchemaObject::MaterializedView(view) = &mut objects[object_index] {
                view.renamed_from = Some(from);
            }
        }
        AttachmentOperation::TableColumn {
            object_index,
            column_index,
            from,
        } => {
            if let SchemaObject::Table(table) = &mut objects[object_index] {
                table.columns[column_index].renamed_from = Some(from);
            }
        }
        AttachmentOperation::MaterializedViewColumn {
            object_index,
            column_index,
            from,
        } => {
            if let SchemaObject::MaterializedView(view) = &mut objects[object_index] {
                view.columns[column_index].renamed_from = Some(from);
            }
        }
    }
}

fn orphan_annotation_error(annotation: &RenameAnnotation) -> crate::Error {
    DiffError::ObjectComparison {
        target: format!(
            "annotation @renamed from={} on line {}",
            format_ident_for_annotation(&annotation.from),
            annotation.line
        ),
        operation: "rename annotation mismatch".to_string(),
    }
    .into()
}

fn format_ident_for_annotation(ident: &Ident) -> String {
    if ident.quoted {
        format!("\"{}\"", ident.value.replace('\"', "\"\""))
    } else {
        ident.value.clone()
    }
}

fn qualified_name_matches(left: &QualifiedName, right: &QualifiedName) -> bool {
    optional_ident_matches(left.schema.as_ref(), right.schema.as_ref())
        && ident_matches(&left.name, &right.name)
}

fn optional_ident_matches(left: Option<&Ident>, right: Option<&Ident>) -> bool {
    match (left, right) {
        (Some(left), Some(right)) => ident_matches(left, right),
        (None, None) => true,
        _ => false,
    }
}

fn ident_matches(left: &Ident, right: &Ident) -> bool {
    if left.quoted || right.quoted {
        left.quoted == right.quoted && left.value == right.value
    } else {
        left.value.eq_ignore_ascii_case(&right.value)
    }
}
