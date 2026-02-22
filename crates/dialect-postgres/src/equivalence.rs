use stateql_core::{EquivalencePolicy, Expr};

#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct PostgresEquivalencePolicy;

pub(crate) static POSTGRES_EQUIVALENCE_POLICY: PostgresEquivalencePolicy =
    PostgresEquivalencePolicy;

impl EquivalencePolicy for PostgresEquivalencePolicy {
    fn is_equivalent_expr(&self, left: &Expr, right: &Expr) -> bool {
        let Some(left_canonical) = canonical_expr(left) else {
            return false;
        };
        let Some(right_canonical) = canonical_expr(right) else {
            return false;
        };

        left_canonical == right_canonical
    }
}

/// Normalization owns structural expression canonicalization in PostgreSQL.
/// This policy is only a residual safety valve for `Expr::Raw` spellings that
/// remain textually different after normalization.
fn canonical_expr(expr: &Expr) -> Option<String> {
    match expr {
        Expr::Raw(raw) => Some(canonical_raw_expr(raw)),
        Expr::Paren(inner) => canonical_expr(inner),
        _ => None,
    }
}

fn canonical_raw_expr(raw: &str) -> String {
    let mut normalized = collapse_whitespace(raw);
    normalized = strip_redundant_outer_parens(normalized);
    normalized = collapse_whitespace(&normalized);

    if let Some(integer_literal) = normalize_integer_cast_literal(&normalized) {
        return integer_literal;
    }
    if let Some(integer_literal) = canonical_integer_literal(&normalized) {
        return integer_literal;
    }

    normalized
}

fn normalize_integer_cast_literal(expr: &str) -> Option<String> {
    let (literal, data_type) = expr.split_once("::")?;
    if data_type.contains("::") {
        return None;
    }

    let canonical_type = canonical_type_name(data_type);
    if !is_integer_type(&canonical_type) {
        return None;
    }

    let quoted_literal = literal.trim();
    let inner = quoted_literal.strip_prefix('\'')?.strip_suffix('\'')?;
    if inner.contains('\'') {
        return None;
    }

    canonical_integer_literal(inner)
}

fn canonical_type_name(raw: &str) -> String {
    raw.rsplit('.')
        .next()
        .unwrap_or(raw)
        .trim()
        .trim_matches('"')
        .to_ascii_lowercase()
}

fn is_integer_type(data_type: &str) -> bool {
    matches!(
        data_type,
        "int" | "int2" | "int4" | "int8" | "integer" | "smallint" | "bigint"
    )
}

fn canonical_integer_literal(raw: &str) -> Option<String> {
    let value = raw.trim();
    if value.is_empty() {
        return None;
    }

    let (negative, digits) = if let Some(rest) = value.strip_prefix('-') {
        (true, rest)
    } else if let Some(rest) = value.strip_prefix('+') {
        (false, rest)
    } else {
        (false, value)
    };

    if digits.is_empty() || !digits.chars().all(|ch| ch.is_ascii_digit()) {
        return None;
    }

    let normalized_digits = digits.trim_start_matches('0');
    let normalized_digits = if normalized_digits.is_empty() {
        "0"
    } else {
        normalized_digits
    };

    if negative && normalized_digits != "0" {
        Some(format!("-{normalized_digits}"))
    } else {
        Some(normalized_digits.to_string())
    }
}

fn strip_redundant_outer_parens(input: String) -> String {
    let mut candidate = input;
    loop {
        let trimmed = candidate.trim();
        if !trimmed.starts_with('(') || !trimmed.ends_with(')') {
            return trimmed.to_string();
        }
        if !outer_parens_wrap_entire_expr(trimmed) {
            return trimmed.to_string();
        }

        let inner = trimmed
            .strip_prefix('(')
            .and_then(|s| s.strip_suffix(')'))
            .unwrap_or(trimmed)
            .trim();
        candidate = inner.to_string();
    }
}

fn outer_parens_wrap_entire_expr(expr: &str) -> bool {
    let mut depth = 0i32;
    let mut in_single_quote = false;
    let mut in_double_quote = false;
    let mut chars = expr.char_indices().peekable();

    while let Some((_index, ch)) = chars.next() {
        if ch == '\'' && !in_double_quote {
            if in_single_quote {
                if matches!(chars.peek(), Some((_, '\''))) {
                    chars.next();
                } else {
                    in_single_quote = false;
                }
            } else {
                in_single_quote = true;
            }
            continue;
        }

        if ch == '"' && !in_single_quote {
            if in_double_quote {
                if matches!(chars.peek(), Some((_, '"'))) {
                    chars.next();
                } else {
                    in_double_quote = false;
                }
            } else {
                in_double_quote = true;
            }
            continue;
        }

        if in_single_quote || in_double_quote {
            continue;
        }

        match ch {
            '(' => depth += 1,
            ')' => {
                depth -= 1;
                if depth < 0 {
                    return false;
                }
                if depth == 0 && chars.peek().is_some() {
                    return false;
                }
            }
            _ => {}
        }
    }

    depth == 0 && !in_single_quote && !in_double_quote
}

fn collapse_whitespace(input: &str) -> String {
    let mut output = String::with_capacity(input.len());
    let mut in_single_quote = false;
    let mut in_double_quote = false;
    let mut pending_space = false;
    let mut chars = input.chars().peekable();

    while let Some(ch) = chars.next() {
        if ch == '\'' && !in_double_quote {
            if pending_space && !output.is_empty() {
                output.push(' ');
            }
            pending_space = false;
            output.push(ch);

            if in_single_quote {
                if matches!(chars.peek(), Some('\'')) {
                    output.push('\'');
                    chars.next();
                } else {
                    in_single_quote = false;
                }
            } else {
                in_single_quote = true;
            }
            continue;
        }

        if ch == '"' && !in_single_quote {
            if pending_space && !output.is_empty() {
                output.push(' ');
            }
            pending_space = false;
            output.push(ch);

            if in_double_quote {
                if matches!(chars.peek(), Some('"')) {
                    output.push('"');
                    chars.next();
                } else {
                    in_double_quote = false;
                }
            } else {
                in_double_quote = true;
            }
            continue;
        }

        if !in_single_quote && !in_double_quote && ch.is_whitespace() {
            pending_space = true;
            continue;
        }

        if pending_space && !output.is_empty() {
            output.push(' ');
        }
        pending_space = false;
        output.push(ch);
    }

    output.trim().to_string()
}
