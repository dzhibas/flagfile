/// Content normalization for the Flagfile formatter.
///
/// Handles operator spacing, comma spacing, boolean case normalization,
/// arrow normalization, and brace spacing — all while skipping content
/// inside quoted strings, regex literals, and `json(...)` bodies.
use super::classify::LineType;

// ── Quote-aware state machine ──────────────────────────────────────

#[derive(Clone, Copy, PartialEq)]
enum QuoteState {
    Normal,
    InSingle,
    InDouble,
    InRegex,
    InJson(usize), // brace depth inside json(...)
}

/// Walk the characters of `input`, calling `process` for each character that
/// is outside of quoted strings, regex literals, and `json(...)` bodies.
/// Characters inside protected regions are copied verbatim to the output.
///
/// `process(output, remaining_input, position)` returns how many bytes to
/// consume (0 means "just copy the current character as-is").
fn walk_unquoted<F>(input: &str, mut process: F) -> String
where
    F: FnMut(&mut String, &str, usize) -> usize,
{
    let bytes = input.as_bytes();
    let len = bytes.len();
    let mut out = String::with_capacity(len + 16);
    let mut state = QuoteState::Normal;
    let mut i = 0;

    while i < len {
        let ch = bytes[i];

        match state {
            QuoteState::InSingle => {
                out.push(ch as char);
                if ch == b'\'' {
                    state = QuoteState::Normal;
                }
                i += 1;
            }
            QuoteState::InDouble => {
                out.push(ch as char);
                if ch == b'"' {
                    state = QuoteState::Normal;
                }
                i += 1;
            }
            QuoteState::InRegex => {
                out.push(ch as char);
                if ch == b'/' {
                    state = QuoteState::Normal;
                }
                i += 1;
            }
            QuoteState::InJson(depth) => {
                out.push(ch as char);
                if ch == b'{' {
                    state = QuoteState::InJson(depth + 1);
                } else if ch == b'}' {
                    if depth <= 1 {
                        // closing paren of json(...) comes next or we just
                        // left the last brace
                        state = QuoteState::Normal;
                    } else {
                        state = QuoteState::InJson(depth - 1);
                    }
                }
                i += 1;
            }
            QuoteState::Normal => {
                // Enter quoted/protected regions
                if ch == b'\'' {
                    state = QuoteState::InSingle;
                    out.push(ch as char);
                    i += 1;
                    continue;
                }
                if ch == b'"' {
                    state = QuoteState::InDouble;
                    out.push(ch as char);
                    i += 1;
                    continue;
                }
                if ch == b'/' && i + 1 < len && bytes[i + 1] != b'/' && bytes[i + 1] != b'*' {
                    // Heuristic: `/` followed by non-slash, non-star is a
                    // regex literal opening.  Check there is something before
                    // it that looks like an operator context (space, `(`, `~`).
                    let prev = if i > 0 { bytes[i - 1] } else { b' ' };
                    if prev == b' ' || prev == b'(' || prev == b'~' || prev == b',' {
                        state = QuoteState::InRegex;
                        out.push(ch as char);
                        i += 1;
                        continue;
                    }
                }
                // Detect `json(` — enter JSON protection
                if ch == b'j' && i + 5 <= len && &input[i..i + 5] == "json(" {
                    // Copy `json(` verbatim, then look for `{`
                    out.push_str("json(");
                    i += 5;
                    // skip any whitespace before `{`
                    while i < len && bytes[i] == b' ' {
                        out.push(' ');
                        i += 1;
                    }
                    if i < len && bytes[i] == b'{' {
                        state = QuoteState::InJson(1);
                        out.push('{');
                        i += 1;
                    }
                    continue;
                }

                // Normal character — let the processor handle it
                let consumed = process(&mut out, &input[i..], i);
                if consumed == 0 {
                    out.push(ch as char);
                    i += 1;
                } else {
                    i += consumed;
                }
            }
        }
    }
    out
}

// ── Public normalization entry point ───────────────────────────────

/// Normalize a trimmed source line based on its classified type.
pub fn normalize_line(trimmed: &str, line_type: &LineType) -> String {
    match line_type {
        LineType::Blank
        | LineType::LineComment
        | LineType::BlockCommentStart
        | LineType::BlockCommentMiddle
        | LineType::BlockCommentEnd
        | LineType::BlockCommentFull => {
            // Comments and blanks are preserved verbatim
            trimmed.to_string()
        }
        LineType::Annotation => {
            // Annotations: just trim, no content changes
            trimmed.to_string()
        }
        LineType::ClosingBrace => "}".to_string(),
        LineType::FlagHeaderBlock => normalize_flag_header_block(trimmed),
        LineType::FlagHeaderShort => normalize_short_form(trimmed),
        LineType::SegmentHeader => normalize_segment_header(trimmed),
        LineType::EnvHeaderBlock => normalize_env_header_block(trimmed),
        LineType::EnvHeaderShort => normalize_short_form(trimmed),
        LineType::RuleExpr | LineType::Continuation => normalize_rule_line(trimmed),
        LineType::StaticValue => normalize_static_value(trimmed),
    }
}

// ── Individual normalizers ─────────────────────────────────────────

/// `FF-name {` — ensure exactly one space before `{`.
fn normalize_flag_header_block(line: &str) -> String {
    if let Some(pos) = line.rfind('{') {
        let name = line[..pos].trim_end();
        format!("{} {{", name)
    } else {
        line.to_string()
    }
}

/// `@segment name {` — ensure spacing.
fn normalize_segment_header(line: &str) -> String {
    if let Some(pos) = line.rfind('{') {
        let before = line[..pos].trim_end();
        format!("{} {{", before)
    } else {
        line.to_string()
    }
}

/// `@env name {` — ensure spacing.
fn normalize_env_header_block(line: &str) -> String {
    if let Some(pos) = line.rfind('{') {
        let before = line[..pos].trim_end();
        format!("{} {{", before)
    } else {
        line.to_string()
    }
}

/// Short form: `FF-name -> value` or `@env name -> value`.
/// Normalizes the arrow and the return value boolean casing.
fn normalize_short_form(line: &str) -> String {
    if let Some((lhs, rhs)) = split_arrow_outside_quotes(line) {
        let rhs_normalized = normalize_return_value(rhs.trim());
        format!("{} -> {}", lhs.trim(), rhs_normalized)
    } else {
        line.to_string()
    }
}

/// Rule line: `expression -> return_value` possibly with a trailing comment.
/// Normalizes operators, commas, the arrow, and the return value.
fn normalize_rule_line(line: &str) -> String {
    // Separate trailing line comment if present
    let (code, trailing_comment) = split_trailing_comment(line);
    let code = code.trim();

    let normalized = if let Some((expr_part, ret_part)) = split_arrow_outside_quotes(code) {
        let expr = normalize_expression(expr_part.trim());
        let ret = normalize_return_value(ret_part.trim());
        format!("{} -> {}", expr, ret)
    } else {
        // Continuation without arrow — just normalize the expression
        normalize_expression(code)
    };

    if let Some(comment) = trailing_comment {
        format!("{} {}", normalized, comment.trim())
    } else {
        normalized
    }
}

/// Bare return value (no condition): `true`, `false`, `42`, `json(...)`, etc.
fn normalize_static_value(line: &str) -> String {
    // Separate trailing line comment if present
    let (code, trailing_comment) = split_trailing_comment(line);
    let normalized = normalize_return_value(code.trim());

    if let Some(comment) = trailing_comment {
        format!("{} {}", normalized, comment.trim())
    } else {
        normalized
    }
}

// ── Expression normalization ───────────────────────────────────────

/// Normalize an expression: operator spacing, comma spacing in lists,
/// then collapse any resulting double-spaces.
fn normalize_expression(expr: &str) -> String {
    let result = normalize_operators(expr);
    let result = normalize_commas(&result);
    collapse_spaces(&result)
}

/// Ensure spaces around comparison and match operators.
///
/// Handles (in longest-match-first order):
///   `!^~`  `!~$`  `^~`  `~$`  `!~`  `~`
///   `<=`  `>=`  `!=`  `==`  `<`  `>`  `=`
///
/// Also normalizes `and` / `or` keywords to have single spaces.
fn normalize_operators(expr: &str) -> String {
    walk_unquoted(expr, |out, remaining, _pos| {
        let bytes = remaining.as_bytes();
        let len = bytes.len();

        // ── Multi-char match operators ──────────────────────────
        // !^~
        if len >= 3 && &remaining[..3] == "!^~" {
            trim_trailing_space(out);
            out.push_str(" !^~ ");
            return 3;
        }
        // !~$
        if len >= 3 && &remaining[..3] == "!~$" {
            trim_trailing_space(out);
            out.push_str(" !~$ ");
            return 3;
        }
        // ^~
        if len >= 2 && &remaining[..2] == "^~" {
            trim_trailing_space(out);
            out.push_str(" ^~ ");
            return 2;
        }
        // ~$
        if len >= 2 && &remaining[..2] == "~$" {
            trim_trailing_space(out);
            out.push_str(" ~$ ");
            return 2;
        }
        // !~  (but not !~$ which is handled above)
        if len >= 2 && &remaining[..2] == "!~" {
            trim_trailing_space(out);
            out.push_str(" !~ ");
            return 2;
        }
        // ~ (standalone tilde, but not as part of ^~ or ~$ or !~)
        if bytes[0] == b'~' {
            trim_trailing_space(out);
            out.push_str(" ~ ");
            return 1;
        }

        // ── Comparison operators ────────────────────────────────
        // <=
        if len >= 2 && &remaining[..2] == "<=" {
            trim_trailing_space(out);
            out.push_str(" <= ");
            return 2;
        }
        // >=
        if len >= 2 && &remaining[..2] == ">=" {
            trim_trailing_space(out);
            out.push_str(" >= ");
            return 2;
        }
        // !=
        if len >= 2 && &remaining[..2] == "!=" {
            trim_trailing_space(out);
            out.push_str(" != ");
            return 2;
        }
        // ==
        if len >= 2 && &remaining[..2] == "==" {
            trim_trailing_space(out);
            out.push_str(" == ");
            return 2;
        }
        // = (standalone, not == or !=  or >=  or <=)
        if bytes[0] == b'=' {
            trim_trailing_space(out);
            out.push_str(" == ");
            return 1;
        }
        // < (standalone, not <=)
        if bytes[0] == b'<' {
            trim_trailing_space(out);
            out.push_str(" < ");
            return 1;
        }
        // > (standalone, not >=, and not ->)
        if bytes[0] == b'>' {
            // Check we aren't the > in ->
            let prev_byte = out.as_bytes().last().copied().unwrap_or(b' ');
            if prev_byte == b'-' {
                // part of ->  — don't treat as operator
                return 0;
            }
            trim_trailing_space(out);
            out.push_str(" > ");
            return 1;
        }

        0 // not consumed — walk_unquoted copies the char
    })
}

/// Normalize comma spacing inside parenthesized lists: `(a,b,c)` → `(a, b, c)`.
fn normalize_commas(expr: &str) -> String {
    let mut paren_depth: usize = 0;

    walk_unquoted(expr, |out, remaining, _pos| {
        let ch = remaining.as_bytes()[0];
        match ch {
            b'(' => {
                paren_depth += 1;
                out.push('(');
                1
            }
            b')' => {
                paren_depth = paren_depth.saturating_sub(1);
                // Remove trailing space before closing paren
                if out.ends_with(' ') && paren_depth == 0 {
                    // Don't trim — this could be expression spacing
                }
                out.push(')');
                1
            }
            b',' if paren_depth > 0 => {
                // Remove any trailing spaces before the comma
                let trimmed_end = out.trim_end_matches(' ');
                let trim_count = out.len() - trimmed_end.len();
                if trim_count > 0 {
                    out.truncate(out.len() - trim_count);
                }
                out.push_str(", ");
                // Skip any whitespace after the comma in the source
                let rest = &remaining[1..];
                let skip = rest.len() - rest.trim_start().len();
                1 + skip
            }
            _ => 0,
        }
    })
}

/// Normalize a return value: boolean case, JSON pretty-print, trim.
fn normalize_return_value(val: &str) -> String {
    match val.to_lowercase().as_str() {
        "true" => return "TRUE".to_string(),
        "false" => return "FALSE".to_string(),
        _ => {}
    }

    // Pretty-print JSON return values using serde_json
    if let Some(json_body) = extract_json_body(val) {
        if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(json_body) {
            // For compact objects (no nesting or few keys), use single-line
            if is_simple_json(&parsed) {
                return format!("json({})", serde_json::to_string(&parsed).unwrap());
            }
            // For complex objects, pretty-print
            return format!("json({})", serde_json::to_string_pretty(&parsed).unwrap());
        }
    }

    val.to_string()
}

/// Extract the JSON body from `json({...})`, returning the `{...}` part.
fn extract_json_body(val: &str) -> Option<&str> {
    let trimmed = val.trim();
    if !trimmed.starts_with("json(") || !trimmed.ends_with(')') {
        return None;
    }
    Some(&trimmed[5..trimmed.len() - 1])
}

/// Check if a JSON value is "simple" enough for single-line rendering:
/// no nested objects/arrays, or very few top-level keys.
fn is_simple_json(val: &serde_json::Value) -> bool {
    match val {
        serde_json::Value::Object(map) => {
            if map.len() > 4 {
                return false;
            }
            map.values().all(|v| {
                matches!(
                    v,
                    serde_json::Value::Null
                        | serde_json::Value::Bool(_)
                        | serde_json::Value::Number(_)
                        | serde_json::Value::String(_)
                )
            })
        }
        serde_json::Value::Array(arr) => {
            arr.len() <= 4
                && arr.iter().all(|v| {
                    matches!(
                        v,
                        serde_json::Value::Null
                            | serde_json::Value::Bool(_)
                            | serde_json::Value::Number(_)
                            | serde_json::Value::String(_)
                    )
                })
        }
        _ => true,
    }
}

// ── Helpers ────────────────────────────────────────────────────────

/// Remove trailing spaces from the output buffer (used before inserting
/// ` op ` to avoid double spaces).
fn trim_trailing_space(out: &mut String) {
    while out.ends_with(' ') {
        out.pop();
    }
}

/// Split a line at the first `->` that is outside of quoted strings.
/// Returns `(lhs, rhs)` with the arrow removed.
fn split_arrow_outside_quotes(line: &str) -> Option<(&str, &str)> {
    let mut in_single = false;
    let mut in_double = false;
    let bytes = line.as_bytes();
    let len = bytes.len();
    let mut i = 0;
    while i < len {
        let ch = bytes[i];
        match ch {
            b'\'' if !in_double => in_single = !in_single,
            b'"' if !in_single => in_double = !in_double,
            b'-' if !in_single && !in_double && i + 1 < len && bytes[i + 1] == b'>' => {
                return Some((&line[..i], &line[i + 2..]));
            }
            _ => {}
        }
        i += 1;
    }
    None
}

/// Split a line into code and trailing `// comment`, respecting quoted strings.
fn split_trailing_comment(line: &str) -> (&str, Option<&str>) {
    let mut in_single = false;
    let mut in_double = false;
    let bytes = line.as_bytes();
    let len = bytes.len();
    let mut i = 0;
    while i < len {
        let ch = bytes[i];
        match ch {
            b'\'' if !in_double => in_single = !in_single,
            b'"' if !in_single => in_double = !in_double,
            b'/' if !in_single && !in_double && i + 1 < len && bytes[i + 1] == b'/' => {
                return (&line[..i], Some(&line[i..]));
            }
            _ => {}
        }
        i += 1;
    }
    (line, None)
}

/// Collapse runs of multiple spaces into single spaces, but only outside
/// of quoted strings. Preserves leading/trailing as-is (caller should trim).
pub fn collapse_spaces(s: &str) -> String {
    walk_unquoted(s, |out, remaining, _pos| {
        if remaining.as_bytes()[0] == b' ' {
            if !out.ends_with(' ') {
                out.push(' ');
            }
            1
        } else {
            0
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── Arrow normalization ──────────────────────────────────────

    #[test]
    fn test_normalize_short_form_arrow() {
        assert_eq!(normalize_short_form("FF-flag->true"), "FF-flag -> TRUE");
        assert_eq!(
            normalize_short_form("FF-flag  ->  false"),
            "FF-flag -> FALSE"
        );
    }

    // ── Operator spacing ────────────────────────────────────────

    #[test]
    fn test_normalize_operators_eq() {
        assert_eq!(normalize_operators("a==b"), "a == b");
        // When source has existing spaces, operators may produce double spaces;
        // these are collapsed by normalize_expression / collapse_spaces later.
        assert_eq!(normalize_operators("a =b"), "a == b");
        assert_eq!(normalize_expression("a = b"), "a == b");
    }

    #[test]
    fn test_normalize_operators_neq() {
        assert_eq!(normalize_operators("a!=b"), "a != b");
    }

    #[test]
    fn test_normalize_operators_gte_lte() {
        assert_eq!(normalize_operators("a>=b"), "a >= b");
        assert_eq!(normalize_operators("a<=b"), "a <= b");
    }

    #[test]
    fn test_normalize_operators_gt_lt() {
        assert_eq!(normalize_operators("a>b"), "a > b");
        assert_eq!(normalize_operators("a<b"), "a < b");
    }

    #[test]
    fn test_normalize_operators_match() {
        assert_eq!(normalize_operators("a~b"), "a ~ b");
        assert_eq!(normalize_operators("a!~b"), "a !~ b");
        assert_eq!(normalize_operators("a^~b"), "a ^~ b");
        assert_eq!(normalize_operators("a~$b"), "a ~$ b");
        assert_eq!(normalize_operators("a!^~b"), "a !^~ b");
        assert_eq!(normalize_operators("a!~$b"), "a !~$ b");
    }

    // ── Comma spacing ───────────────────────────────────────────

    #[test]
    fn test_normalize_commas() {
        assert_eq!(normalize_commas("(a,b,c)"), "(a, b, c)");
        assert_eq!(normalize_commas("(a , b , c)"), "(a, b, c)");
        assert_eq!(normalize_commas("( a,b )"), "( a, b )");
    }

    // ── Boolean normalization ───────────────────────────────────

    #[test]
    fn test_normalize_return_value_bool() {
        assert_eq!(normalize_return_value("true"), "TRUE");
        assert_eq!(normalize_return_value("false"), "FALSE");
        assert_eq!(normalize_return_value("True"), "TRUE");
        assert_eq!(normalize_return_value("FALSE"), "FALSE");
    }

    #[test]
    fn test_normalize_return_value_non_bool() {
        assert_eq!(normalize_return_value("5000"), "5000");
        assert_eq!(normalize_return_value("\"debug\""), "\"debug\"");
    }

    // ── Static value normalization ──────────────────────────────

    #[test]
    fn test_normalize_static_value() {
        assert_eq!(normalize_static_value("true"), "TRUE");
        assert_eq!(
            normalize_static_value("false // comment"),
            "FALSE // comment"
        );
    }

    // ── Rule line normalization ─────────────────────────────────

    #[test]
    fn test_normalize_rule_line_basic() {
        assert_eq!(normalize_rule_line("a==b -> true"), "a == b -> TRUE");
    }

    #[test]
    fn test_normalize_rule_with_trailing_comment() {
        assert_eq!(
            normalize_rule_line("lower(name) ~ nik -> true // contains"),
            "lower(name) ~ nik -> TRUE // contains"
        );
        // Compact form gets spaced out
        assert_eq!(
            normalize_rule_line("lower(name)~nik -> true // contains"),
            "lower(name) ~ nik -> TRUE // contains"
        );
    }

    // ── json protection ─────────────────────────────────────────

    #[test]
    fn test_normalize_operators_skips_quotes() {
        assert_eq!(normalize_expression("a == \"x>=y\""), "a == \"x>=y\"");
        assert_eq!(normalize_expression("a == 'x!=y'"), "a == 'x!=y'");
    }

    #[test]
    fn test_normalize_operators_skips_regex() {
        assert_eq!(normalize_expression("name ~ /.*ola.*/"), "name ~ /.*ola.*/");
    }

    // ── Full line normalization ─────────────────────────────────

    #[test]
    fn test_normalize_line_flag_header_block() {
        assert_eq!(
            normalize_line("FF-my-flag  {", &LineType::FlagHeaderBlock),
            "FF-my-flag {"
        );
    }

    #[test]
    fn test_json_simple_compacted() {
        // Simple JSON is compacted to single line via serde_json
        assert_eq!(
            normalize_static_value("json({\"key\":  \"val\"})"),
            "json({\"key\":\"val\"})"
        );
    }

    #[test]
    fn test_json_nested_braces() {
        // Nested JSON gets pretty-printed via serde_json
        let input = "json({\"a\": {\"b\": 1}})";
        let result = normalize_static_value(input);
        assert!(result.starts_with("json({"), "got: {}", result);
        assert!(result.contains("\"a\""), "got: {}", result);
        // Should contain newlines (pretty-printed)
        assert!(
            result.contains('\n'),
            "should be pretty-printed, got: {}",
            result
        );
    }

    #[test]
    fn test_json_empty_object() {
        assert_eq!(normalize_static_value("json({})"), "json({})");
    }

    // ── split_trailing_comment ──────────────────────────────────

    #[test]
    fn test_split_trailing_comment() {
        let (code, comment) = split_trailing_comment("expr -> true // reason");
        assert_eq!(code, "expr -> true ");
        assert_eq!(comment, Some("// reason"));
    }

    #[test]
    fn test_split_trailing_comment_in_string() {
        let (code, comment) = split_trailing_comment("a == \"hello // world\"");
        assert_eq!(code, "a == \"hello // world\"");
        assert_eq!(comment, None);
    }

    // ── collapse_spaces ─────────────────────────────────────────

    #[test]
    fn test_collapse_spaces() {
        assert_eq!(collapse_spaces("a  ==  b"), "a == b");
        assert_eq!(collapse_spaces("a  ==  \"x  y\""), "a == \"x  y\"");
    }
}
