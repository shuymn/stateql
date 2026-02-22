use crate::{Ident, Result};

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
