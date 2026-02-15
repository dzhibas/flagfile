/// Line classification for the Flagfile formatter.
///
/// Each source line is assigned a `LineType` that determines its indentation
/// level and whether its content should be normalized.

#[derive(Debug, Clone, PartialEq)]
pub enum LineType {
    Blank,
    LineComment,
    BlockCommentStart,
    BlockCommentMiddle,
    BlockCommentEnd,
    BlockCommentFull,
    Annotation,
    FlagHeaderBlock,
    FlagHeaderShort,
    SegmentHeader,
    EnvHeaderBlock,
    EnvHeaderShort,
    ClosingBrace,
    RuleExpr,
    StaticValue,
    Continuation,
}

/// Classify a single (trimmed) source line.
///
/// `in_block_comment` – whether we are currently inside a `/* ... */` block.
/// `prev_expects_continuation` – whether the previous meaningful line started
/// a rule expression that has not yet reached its `->` arrow.
pub fn classify_line(
    trimmed: &str,
    in_block_comment: bool,
    prev_expects_continuation: bool,
) -> LineType {
    // ── Inside a block comment ──────────────────────────────────────
    if in_block_comment {
        if trimmed.contains("*/") {
            return LineType::BlockCommentEnd;
        }
        return LineType::BlockCommentMiddle;
    }

    // ── Blank line ──────────────────────────────────────────────────
    if trimmed.is_empty() {
        return LineType::Blank;
    }

    // ── Block comment that opens (and maybe closes) on this line ───
    if trimmed.starts_with("/*") || trimmed.starts_with("/**") {
        if trimmed.contains("*/") {
            return LineType::BlockCommentFull;
        }
        return LineType::BlockCommentStart;
    }

    // ── Line comment ────────────────────────────────────────────────
    if trimmed.starts_with("//") {
        return LineType::LineComment;
    }

    // ── Closing brace ───────────────────────────────────────────────
    if trimmed == "}" {
        return LineType::ClosingBrace;
    }

    // ── Segment header ──────────────────────────────────────────────
    if trimmed.starts_with("@segment") || trimmed.starts_with("@segment ") {
        return LineType::SegmentHeader;
    }

    // ── @env rules ──────────────────────────────────────────────────
    if trimmed.starts_with("@env ") {
        if ends_with_block_brace(trimmed) {
            return LineType::EnvHeaderBlock;
        }
        if contains_arrow_outside_quotes(trimmed) {
            return LineType::EnvHeaderShort;
        }
        // fallthrough – unusual, treat as annotation
    }

    // ── Metadata annotations ────────────────────────────────────────
    if let Some(after_at) = trimmed.strip_prefix('@') {
        if after_at.starts_with("owner")
            || after_at.starts_with("expires")
            || after_at.starts_with("ticket")
            || after_at.starts_with("description")
            || after_at.starts_with("type")
            || after_at.starts_with("deprecated")
            || after_at.starts_with("requires")
            || after_at.starts_with("test")
        {
            return LineType::Annotation;
        }
    }

    // ── Flag header ─────────────────────────────────────────────────
    if trimmed.starts_with("FF-") || trimmed.starts_with("FF_") {
        if ends_with_block_brace(trimmed) {
            return LineType::FlagHeaderBlock;
        }
        if contains_arrow_outside_quotes(trimmed) {
            return LineType::FlagHeaderShort;
        }
        // Flag name with no body – treat as static value
        return LineType::StaticValue;
    }

    // ── Continuation of a multi-line expression ─────────────────────
    if prev_expects_continuation {
        return LineType::Continuation;
    }

    // ── Rule expression (has ->) vs. static value ───────────────────
    if contains_arrow_outside_quotes(trimmed) {
        return LineType::RuleExpr;
    }

    LineType::StaticValue
}

/// Check whether the trimmed line ends with `{` as a block opener.
///
/// This distinguishes `FF-flag {` (block) from `FF-flag -> json({"a":1})`
/// (short form with JSON containing braces).
fn ends_with_block_brace(trimmed: &str) -> bool {
    trimmed.trim_end().ends_with('{') && !trimmed.contains("json(")
}

/// Check whether `line` contains ` -> ` (or `->`) outside of quoted strings.
fn contains_arrow_outside_quotes(line: &str) -> bool {
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
                return true;
            }
            _ => {}
        }
        i += 1;
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_classify_blank() {
        assert_eq!(classify_line("", false, false), LineType::Blank);
    }

    #[test]
    fn test_classify_line_comment() {
        assert_eq!(
            classify_line("// hello", false, false),
            LineType::LineComment
        );
    }

    #[test]
    fn test_classify_block_comment_full() {
        assert_eq!(
            classify_line("/* comment */", false, false),
            LineType::BlockCommentFull
        );
    }

    #[test]
    fn test_classify_block_comment_start() {
        assert_eq!(
            classify_line("/* comment", false, false),
            LineType::BlockCommentStart
        );
    }

    #[test]
    fn test_classify_block_comment_middle() {
        assert_eq!(
            classify_line("some text", true, false),
            LineType::BlockCommentMiddle
        );
    }

    #[test]
    fn test_classify_block_comment_end() {
        assert_eq!(
            classify_line("end */", true, false),
            LineType::BlockCommentEnd
        );
    }

    #[test]
    fn test_classify_annotation() {
        assert_eq!(
            classify_line("@owner \"team\"", false, false),
            LineType::Annotation
        );
        assert_eq!(
            classify_line("@expires 2027-01-01", false, false),
            LineType::Annotation
        );
        assert_eq!(
            classify_line("@test FF-foo == true", false, false),
            LineType::Annotation
        );
        assert_eq!(
            classify_line("@requires FF-bar", false, false),
            LineType::Annotation
        );
    }

    #[test]
    fn test_classify_flag_header_block() {
        assert_eq!(
            classify_line("FF-my-flag {", false, false),
            LineType::FlagHeaderBlock
        );
    }

    #[test]
    fn test_classify_flag_header_short() {
        assert_eq!(
            classify_line("FF-my-flag -> true", false, false),
            LineType::FlagHeaderShort
        );
    }

    #[test]
    fn test_classify_segment_header() {
        assert_eq!(
            classify_line("@segment my_seg {", false, false),
            LineType::SegmentHeader
        );
    }

    #[test]
    fn test_classify_env_header_block() {
        assert_eq!(
            classify_line("@env stage {", false, false),
            LineType::EnvHeaderBlock
        );
    }

    #[test]
    fn test_classify_env_header_short() {
        assert_eq!(
            classify_line("@env dev -> true", false, false),
            LineType::EnvHeaderShort
        );
    }

    #[test]
    fn test_classify_closing_brace() {
        assert_eq!(classify_line("}", false, false), LineType::ClosingBrace);
    }

    #[test]
    fn test_classify_rule_expr() {
        assert_eq!(
            classify_line("a == b -> true", false, false),
            LineType::RuleExpr
        );
    }

    #[test]
    fn test_classify_static_value() {
        assert_eq!(classify_line("false", false, false), LineType::StaticValue);
        assert_eq!(
            classify_line("json({})", false, false),
            LineType::StaticValue
        );
    }

    #[test]
    fn test_classify_continuation() {
        assert_eq!(
            classify_line("and demo == false -> TRUE", false, true),
            LineType::Continuation
        );
    }

    #[test]
    fn test_arrow_inside_quotes_not_detected() {
        // Arrow inside a string should not count
        assert_eq!(
            classify_line("name == \"a -> b\"", false, false),
            LineType::StaticValue
        );
    }

    #[test]
    fn test_classify_docblock_start() {
        assert_eq!(
            classify_line("/**", false, false),
            LineType::BlockCommentStart
        );
    }
}
