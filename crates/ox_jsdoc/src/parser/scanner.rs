// @author kazuya kawaguchi (a.k.a. kazupon)
// @license MIT
//

//! Low-level scanner helpers for turning a raw block comment into logical
//! content lines.

/// One source line after stripping the comment prefix syntax.
///
/// `content_start` and `content_end` remain absolute byte offsets so later AST
/// nodes can point back to the original input.
#[derive(Debug, Clone, Copy)]
pub struct LogicalLine<'a> {
    /// Content after removing the visual JSDoc margin.
    pub content: &'a str,
    /// Absolute byte offset where `content` starts.
    pub content_start: u32,
    /// Absolute byte offset where the original physical line ends.
    pub content_end: u32,
}

/// JSDoc blocks must start with `/**`; plain `/*` comments are rejected.
pub fn is_jsdoc_block(source_text: &str) -> bool {
    source_text.starts_with("/**")
}

/// The parser currently accepts only complete block comments.
pub fn has_closing_block(source_text: &str) -> bool {
    source_text.ends_with("*/")
}

/// Return the byte range between the opening `/**` and closing `*/`.
pub fn body_range(source_text: &str) -> Option<(usize, usize)> {
    if !is_jsdoc_block(source_text) || !has_closing_block(source_text) || source_text.len() < 5 {
        return None;
    }

    Some((3, source_text.len() - 2))
}

/// Split the comment body into content lines.
///
/// This removes the conventional leading whitespace, `*`, and one following
/// space/tab from each line, but it keeps trailing content unchanged so spans
/// and raw tag bodies remain faithful to source.
pub fn logical_lines(source_text: &str, base_offset: u32) -> Vec<LogicalLine<'_>> {
    let Some((body_start, body_end)) = body_range(source_text) else {
        return Vec::new();
    };

    let body = &source_text[body_start..body_end];
    let mut lines = Vec::new();
    let mut cursor = 0usize;

    while cursor <= body.len() {
        let line_end = body[cursor..]
            .find('\n')
            .map(|index| cursor + index)
            .unwrap_or(body.len());
        let raw_line = &body[cursor..line_end];
        let raw_start = body_start + cursor;

        // Strip the visual JSDoc margin: optional indentation, leading `*`,
        // then at most one separator space.
        let mut content_start = 0usize;
        let bytes = raw_line.as_bytes();
        while content_start < bytes.len() && matches!(bytes[content_start], b' ' | b'\t') {
            content_start += 1;
        }
        if bytes.get(content_start) == Some(&b'*') {
            content_start += 1;
            if matches!(bytes.get(content_start), Some(b' ' | b'\t')) {
                content_start += 1;
            }
        }

        let absolute_content_start =
            base_offset + u32::try_from(raw_start + content_start).unwrap();
        let absolute_content_end = base_offset + u32::try_from(raw_start + raw_line.len()).unwrap();
        lines.push(LogicalLine {
            content: &raw_line[content_start..],
            content_start: absolute_content_start,
            content_end: absolute_content_end,
        });

        if line_end == body.len() {
            break;
        }
        cursor = line_end + 1;
    }

    lines
}

#[cfg(test)]
mod tests {
    use super::{body_range, has_closing_block, is_jsdoc_block, logical_lines};

    #[test]
    fn recognizes_only_closed_jsdoc_blocks() {
        assert!(is_jsdoc_block("/** ok */"));
        assert!(!is_jsdoc_block("/* plain */"));

        assert!(has_closing_block("/** ok */"));
        assert!(!has_closing_block("/** unclosed"));

        assert_eq!(body_range("/** ok */"), Some((3, 7)));
        assert_eq!(body_range("/* plain */"), None);
        assert_eq!(body_range("/** unclosed"), None);
    }

    #[test]
    fn strips_jsdoc_margin_and_keeps_absolute_offsets() {
        let source = "/**\n * Find a user.\n * @param {string} id\n */";
        let lines = logical_lines(source, 100);

        assert_eq!(lines.len(), 4);
        assert_eq!(lines[0].content, "");
        assert_eq!(lines[0].content_start, 103);
        assert_eq!(lines[0].content_end, 103);

        assert_eq!(lines[1].content, "Find a user.");
        assert_eq!(lines[1].content_start, 107);
        assert_eq!(lines[1].content_end, 119);

        assert_eq!(lines[2].content, "@param {string} id");
        assert_eq!(lines[2].content_start, 123);
        assert_eq!(lines[2].content_end, 141);

        assert_eq!(lines[3].content, "");
        assert_eq!(lines[3].content_start, 143);
        assert_eq!(lines[3].content_end, 143);
    }

    #[test]
    fn preserves_trailing_body_text() {
        let source = "/** value   */";
        let lines = logical_lines(source, 0);

        assert_eq!(lines.len(), 1);
        assert_eq!(lines[0].content, "value   ");
        assert_eq!(lines[0].content_start, 4);
        assert_eq!(lines[0].content_end, 12);
    }

    #[test]
    fn returns_no_lines_for_invalid_block_shells() {
        assert!(logical_lines("/* plain */", 0).is_empty());
        assert!(logical_lines("/** unclosed", 0).is_empty());
    }
}
