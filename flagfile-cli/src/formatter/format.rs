/// Core formatting algorithm for the Flagfile formatter.
///
/// Processes input text line-by-line, tracking brace depth for indentation,
/// classifying each line, and normalizing its content.
use super::classify::{classify_line, LineType};
use super::normalize::{collapse_spaces, normalize_line};

const INDENT: usize = 4;

/// Heuristic: does this line look like it's part of a boolean expression
/// rather than a static return value?
///
/// Static values are: `true`, `false`, `TRUE`, `FALSE`, integers, quoted
/// strings, `json(...)`. Anything containing comparison operators, `and`,
/// `or`, `in`, `not`, function calls, or variables with operators is
/// likely an expression.
fn looks_like_expression(trimmed: &str) -> bool {
    // Quick check: known static values are never expressions
    let lower = trimmed.to_lowercase();
    if lower == "true" || lower == "false" {
        return false;
    }
    if trimmed.starts_with('"') || trimmed.starts_with('\'') {
        return false;
    }
    if trimmed.starts_with("json(") {
        return false;
    }
    if trimmed.parse::<i64>().is_ok() {
        return false;
    }

    // If it contains comparison/logic operators or function-like syntax,
    // it's an expression.
    let has_operator = trimmed.contains("==")
        || trimmed.contains("!=")
        || trimmed.contains(">=")
        || trimmed.contains("<=")
        || trimmed.contains(" > ")
        || trimmed.contains(" < ")
        || trimmed.contains(" ~ ")
        || trimmed.contains("!~")
        || trimmed.contains("^~")
        || trimmed.contains("~$");

    // Check for logic/array keywords as whole words (surrounded by spaces)
    let has_keyword = contains_word(trimmed, "and")
        || contains_word(trimmed, "or")
        || contains_word(trimmed, "in")
        || contains_word(trimmed, "not");

    let has_function = trimmed.contains("segment(")
        || trimmed.contains("percentage(")
        || trimmed.contains("coalesce(")
        || trimmed.contains("lower(")
        || trimmed.contains("upper(")
        || trimmed.contains("LOWER(")
        || trimmed.contains("UPPER(")
        || trimmed.contains("now(")
        || trimmed.contains("NOW(");

    has_operator || has_keyword || has_function
}

/// Check if `word` appears as a whole word in `text` (surrounded by
/// non-alphanumeric characters or at string boundaries).
fn contains_word(text: &str, word: &str) -> bool {
    for (i, _) in text.match_indices(word) {
        let before_ok = i == 0 || !text.as_bytes()[i - 1].is_ascii_alphanumeric();
        let after = i + word.len();
        let after_ok = after >= text.len() || !text.as_bytes()[after].is_ascii_alphanumeric();
        if before_ok && after_ok {
            return true;
        }
    }
    false
}

/// Strip up to `max_strip` leading spaces from a line, preserving any
/// additional internal indentation (e.g. the ` * ` in JSDoc block comments).
fn strip_indent(line: &str, max_strip: usize) -> &str {
    let bytes = line.as_bytes();
    let mut stripped = 0;
    while stripped < max_strip && stripped < bytes.len() && bytes[stripped] == b' ' {
        stripped += 1;
    }
    &line[stripped..]
}

/// Format a Flagfile source string, returning the formatted version.
pub fn format_flagfile(input: &str) -> String {
    let lines: Vec<&str> = input.lines().collect();
    let mut output: Vec<String> = Vec::with_capacity(lines.len());
    let mut depth: usize = 0;
    let mut in_block_comment = false;
    let mut prev_expects_continuation = false;
    let mut prev_was_blank = false;
    let mut prev_was_open_brace = false;

    for line in &lines {
        let trimmed = line.trim();
        let line_type = classify_line(trimmed, in_block_comment, prev_expects_continuation);

        // ── Update block-comment tracking ──────────────────────
        match line_type {
            LineType::BlockCommentStart => in_block_comment = true,
            LineType::BlockCommentEnd => in_block_comment = false,
            _ => {}
        }

        // ── Handle blank lines ─────────────────────────────────
        if line_type == LineType::Blank {
            // Suppress blank lines right after an opening brace
            if prev_was_open_brace {
                continue;
            }
            // Collapse consecutive blanks to at most one
            if prev_was_blank {
                continue;
            }
            // Don't emit a blank as the very first line
            if output.is_empty() {
                continue;
            }
            prev_was_blank = true;
            output.push(String::new());
            prev_expects_continuation = false;
            prev_was_open_brace = false;
            continue;
        }

        // ── Suppress blank line before a closing brace ─────────
        if line_type == LineType::ClosingBrace && prev_was_blank {
            // Remove the trailing blank we already emitted
            if let Some(last) = output.last() {
                if last.is_empty() {
                    output.pop();
                }
            }
        }

        prev_was_blank = false;

        // ── Adjust depth BEFORE output for closing braces ──────
        if line_type == LineType::ClosingBrace {
            depth = depth.saturating_sub(1);
        }

        // ── Compute indentation ────────────────────────────────
        let indent = match line_type {
            LineType::Continuation => (depth + 1) * INDENT,
            _ => depth * INDENT,
        };

        // ── Normalize content ──────────────────────────────────
        let normalized = match line_type {
            // Block comment internals: preserve the ` * ` prefix structure.
            // Strip only whitespace up to the current indent level, keeping
            // any extra spaces that form the comment's internal formatting.
            LineType::BlockCommentMiddle | LineType::BlockCommentEnd => {
                strip_indent(line, indent).to_string()
            }
            _ => normalize_line(trimmed, &line_type),
        };
        // Collapse any remaining double-spaces in non-comment content
        let normalized = match line_type {
            LineType::LineComment
            | LineType::BlockCommentStart
            | LineType::BlockCommentMiddle
            | LineType::BlockCommentEnd
            | LineType::BlockCommentFull
            | LineType::Blank => normalized,
            _ => collapse_spaces(&normalized),
        };

        let formatted = if normalized.is_empty() {
            String::new()
        } else {
            format!("{}{}", " ".repeat(indent), normalized)
        };
        output.push(formatted);

        // ── Adjust depth AFTER output for opening braces ───────
        prev_was_open_brace = false;
        match line_type {
            LineType::FlagHeaderBlock | LineType::SegmentHeader | LineType::EnvHeaderBlock => {
                depth += 1;
                prev_was_open_brace = true;
            }
            _ => {}
        }

        // ── Track continuation state ───────────────────────────
        // A line expects continuation when it looks like the start (or middle)
        // of a conditional rule expression that hasn't reached `->` yet.
        prev_expects_continuation = match line_type {
            LineType::RuleExpr | LineType::Continuation => !trimmed.contains("->"),
            // A StaticValue inside a block might actually be the start of a
            // multi-line expression (e.g. `model in (...) and created >= ...`).
            // Detect this by checking for expression-like content.
            LineType::StaticValue if depth > 0 => {
                looks_like_expression(trimmed) && !trimmed.contains("->")
            }
            _ => false,
        };
    }

    // Remove trailing blank lines
    while output.last().is_some_and(|l| l.is_empty()) {
        output.pop();
    }

    // Ensure final newline
    let mut result = output.join("\n");
    result.push('\n');
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── Basic indentation ──────────────────────────────────────

    #[test]
    fn test_simple_short_flag() {
        let input = "FF-flag -> true\n";
        let expected = "FF-flag -> TRUE\n";
        assert_eq!(format_flagfile(input), expected);
    }

    #[test]
    fn test_block_flag_indentation() {
        let input = "\
FF-my-flag {
a == b -> true
false
}
";
        let expected = "\
FF-my-flag {
    a == b -> TRUE
    FALSE
}
";
        assert_eq!(format_flagfile(input), expected);
    }

    #[test]
    fn test_nested_env_indentation() {
        let input = "\
FF-flag {
@env stage {
appVersion >= 5.3 -> false
false
}
@env dev -> true
false
}
";
        let expected = "\
FF-flag {
    @env stage {
        appVersion >= 5.3 -> FALSE
        FALSE
    }
    @env dev -> TRUE
    FALSE
}
";
        assert_eq!(format_flagfile(input), expected);
    }

    // ── Blank line handling ────────────────────────────────────

    #[test]
    fn test_collapse_multiple_blanks() {
        let input = "\
FF-a -> true



FF-b -> false
";
        let expected = "\
FF-a -> TRUE

FF-b -> FALSE
";
        assert_eq!(format_flagfile(input), expected);
    }

    #[test]
    fn test_no_blank_after_open_brace() {
        let input = "\
FF-flag {

    a == b -> true
    false
}
";
        let expected = "\
FF-flag {
    a == b -> TRUE
    FALSE
}
";
        assert_eq!(format_flagfile(input), expected);
    }

    #[test]
    fn test_no_blank_before_close_brace() {
        let input = "\
FF-flag {
    a == b -> true
    false

}
";
        let expected = "\
FF-flag {
    a == b -> TRUE
    FALSE
}
";
        assert_eq!(format_flagfile(input), expected);
    }

    // ── Comment preservation ───────────────────────────────────

    #[test]
    fn test_comment_preserved() {
        let input = "\
// This is a comment
FF-flag -> true
";
        let expected = "\
// This is a comment
FF-flag -> TRUE
";
        assert_eq!(format_flagfile(input), expected);
    }

    #[test]
    fn test_comment_inside_block_indented() {
        let input = "\
FF-flag {
// inner comment
a == b -> true
false
}
";
        let expected = "\
FF-flag {
    // inner comment
    a == b -> TRUE
    FALSE
}
";
        assert_eq!(format_flagfile(input), expected);
    }

    #[test]
    fn test_block_comment_preserved() {
        let input = "\
/**
 * This is a docblock
 * @test FF-flag == true
 */
FF-flag -> true
";
        let expected = "\
/**
 * This is a docblock
 * @test FF-flag == true
 */
FF-flag -> TRUE
";
        assert_eq!(format_flagfile(input), expected);
    }

    #[test]
    fn test_block_comment_inside_flag_indented() {
        let input = "\
FF-flag {
/* comment like this */
true
}
";
        let expected = "\
FF-flag {
    /* comment like this */
    TRUE
}
";
        assert_eq!(format_flagfile(input), expected);
    }

    // ── Operator normalization ─────────────────────────────────

    #[test]
    fn test_operator_spacing_in_rule() {
        let input = "\
FF-flag {
    a==b -> true
    false
}
";
        let expected = "\
FF-flag {
    a == b -> TRUE
    FALSE
}
";
        assert_eq!(format_flagfile(input), expected);
    }

    // ── Boolean normalization ──────────────────────────────────

    #[test]
    fn test_boolean_uppercased_in_return() {
        let input = "FF-flag -> false\n";
        assert_eq!(format_flagfile(input), "FF-flag -> FALSE\n");
    }

    #[test]
    fn test_boolean_uppercased_static() {
        let input = "\
FF-flag {
    true
}
";
        let expected = "\
FF-flag {
    TRUE
}
";
        assert_eq!(format_flagfile(input), expected);
    }

    // ── Comma normalization ────────────────────────────────────

    #[test]
    fn test_comma_spacing_in_list() {
        let input = "\
FF-flag {
    a in (1,2,3) -> true
    false
}
";
        let expected = "\
FF-flag {
    a in (1, 2, 3) -> TRUE
    FALSE
}
";
        assert_eq!(format_flagfile(input), expected);
    }

    // ── Annotation handling ────────────────────────────────────

    #[test]
    fn test_annotations_at_flag_level() {
        let input = "\
@owner \"team\"
@expires 2027-01-01
FF-flag -> true
";
        let expected = "\
@owner \"team\"
@expires 2027-01-01
FF-flag -> TRUE
";
        assert_eq!(format_flagfile(input), expected);
    }

    // ── Segment formatting ─────────────────────────────────────

    #[test]
    fn test_segment_formatting() {
        let input = "\
@segment my_seg{
a == b and c == d
}
";
        let expected = "\
@segment my_seg {
    a == b and c == d
}
";
        assert_eq!(format_flagfile(input), expected);
    }

    // ── Multi-line expression continuation ─────────────────────

    #[test]
    fn test_continuation_line_indentation() {
        let input = "\
FF-flag {
model in (ms,mx,m3,my) and created >= 2024-01-01
and demo == false -> TRUE
false
}
";
        let expected = "\
FF-flag {
    model in (ms,mx,m3,my) and created >= 2024-01-01
        and demo == false -> TRUE
    FALSE
}
";
        assert_eq!(format_flagfile(input), expected);
    }

    // ── JSON content preserved ─────────────────────────────────

    #[test]
    fn test_json_simple_compacted() {
        let input = "\
FF-theme -> json({\"a\":  1, \"b\":  2})
";
        let expected = "\
FF-theme -> json({\"a\":1,\"b\":2})
";
        assert_eq!(format_flagfile(input), expected);
    }

    #[test]
    fn test_json_empty() {
        let input = "FF-flag -> json({})\n";
        assert_eq!(format_flagfile(input), "FF-flag -> json({})\n");
    }

    // ── Trailing inline comment ────────────────────────────────

    #[test]
    fn test_trailing_comment_preserved() {
        let input = "\
FF-flag {
    lower(name) ~ nik -> true // contains
    false
}
";
        let expected = "\
FF-flag {
    lower(name) ~ nik -> TRUE // contains
    FALSE
}
";
        assert_eq!(format_flagfile(input), expected);
    }

    // ── Idempotency ────────────────────────────────────────────

    #[test]
    fn test_idempotent() {
        let input = "\
// Basic on/off switches
FF-new-ui -> TRUE
FF-beta-features -> FALSE

// Feature with rules
@owner \"team\"
FF-feature-y {
    lower(countryCode) == nl -> TRUE
    FALSE
}

@segment complex_segment_test {
    a == b and c == d and (dd not in (1, 2, 3) or z == \"demo car\")
}
";
        let formatted = format_flagfile(input);
        let formatted_again = format_flagfile(&formatted);
        assert_eq!(formatted, formatted_again, "Formatter is not idempotent");
    }

    // ── Trailing whitespace removed ────────────────────────────

    #[test]
    fn test_trailing_whitespace_removed() {
        let input = "FF-flag -> true   \n";
        let result = format_flagfile(input);
        assert!(!result.contains("   \n"), "Trailing whitespace not removed");
        assert_eq!(result, "FF-flag -> TRUE\n");
    }

    // ── Final newline ensured ──────────────────────────────────

    #[test]
    fn test_final_newline() {
        let input = "FF-flag -> true";
        let result = format_flagfile(input);
        assert!(result.ends_with('\n'));
    }

    // ── Leading blank lines removed ────────────────────────────

    #[test]
    fn test_leading_blanks_removed() {
        let input = "\n\n\nFF-flag -> true\n";
        assert_eq!(format_flagfile(input), "FF-flag -> TRUE\n");
    }

    // ── Env short form ─────────────────────────────────────────

    #[test]
    fn test_env_short_form() {
        let input = "\
FF-flag {
    @env dev->true
    false
}
";
        let expected = "\
FF-flag {
    @env dev -> TRUE
    FALSE
}
";
        assert_eq!(format_flagfile(input), expected);
    }

    // ── Arrow spacing ──────────────────────────────────────────

    #[test]
    fn test_arrow_spacing_normalized() {
        let input = "FF-flag->true\n";
        assert_eq!(format_flagfile(input), "FF-flag -> TRUE\n");

        let input2 = "FF-flag  ->  true\n";
        assert_eq!(format_flagfile(input2), "FF-flag -> TRUE\n");
    }

    // ── Blank line between top-level entries ───────────────────

    #[test]
    fn test_blank_line_between_entries_preserved() {
        let input = "\
FF-a -> TRUE

FF-b -> FALSE
";
        // The blank line between the two flags should be preserved
        assert_eq!(format_flagfile(input), input);
    }

    // ── Complex real-world scenario ────────────────────────────

    #[test]
    fn test_complex_scenario() {
        let input = "\
// comment
@expires 2027-01-01
@owner \"Nikolajus\"
FF-sdk-upgrade {
@env stage {
appVersion>=5.3.42 -> false
appVersion<4.32.0 -> false
false
}
@env dev -> true
appVersion >= 5.3.42 -> true
false
}
";
        let expected = "\
// comment
@expires 2027-01-01
@owner \"Nikolajus\"
FF-sdk-upgrade {
    @env stage {
        appVersion >= 5.3.42 -> FALSE
        appVersion < 4.32.0 -> FALSE
        FALSE
    }
    @env dev -> TRUE
    appVersion >= 5.3.42 -> TRUE
    FALSE
}
";
        assert_eq!(format_flagfile(input), expected);
    }
}
