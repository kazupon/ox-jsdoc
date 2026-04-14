// @author kazuya kawaguchi (a.k.a. kazupon)
// @license MIT
//

//! Low-level scanner helpers for turning a raw block comment into logical
//! content lines.

/// One source line after stripping the comment prefix syntax.
///
/// Kept small (24 bytes) so that `Copy` in hot loops is cheap.
/// Margin metadata lives in the parallel `MarginInfo` array.
#[derive(Debug, Clone, Copy)]
pub struct LogicalLine<'a> {
    /// Content after removing the visual JSDoc margin.
    pub content: &'a str,
    /// Absolute byte offset where `content` starts.
    pub content_start: u32,
    /// Absolute byte offset where the original physical line ends.
    pub content_end: u32,
}

/// Source-preserving margin metadata for one logical line.
///
/// Stored in a parallel array alongside `LogicalLine` and only accessed
/// when building compat-mode output or description lines.
#[derive(Debug, Clone, Copy)]
pub struct MarginInfo<'a> {
    /// Indentation before the `*` delimiter (spaces/tabs).
    pub initial: &'a str,
    /// The `*` delimiter itself, or `""` if absent.
    pub delimiter: &'a str,
    /// Whitespace after the `*` delimiter (at most one space/tab), or `""`.
    pub post_delimiter: &'a str,
    /// Line ending characters (`"\n"`, `"\r\n"`, or `""`).
    pub line_end: &'a str,
    /// Whether content (after margin stripping) is empty or whitespace-only.
    pub is_content_empty: bool,
}

/// Result of `logical_lines()`: parallel arrays of content and margin info.
pub struct ScanResult<'a> {
    pub lines: Vec<LogicalLine<'a>>,
    pub margins: Vec<MarginInfo<'a>>,
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

/// Split the comment body into content lines with parallel margin metadata.
///
/// This removes the conventional leading whitespace, `*`, and one following
/// space/tab from each line, but it keeps trailing content unchanged so spans
/// and raw tag bodies remain faithful to source.
pub fn logical_lines(source_text: &str, base_offset: u32) -> ScanResult<'_> {
    let Some((body_start, body_end)) = body_range(source_text) else {
        return ScanResult {
            lines: Vec::new(),
            margins: Vec::new(),
        };
    };

    let body = &source_text[body_start..body_end];
    let mut lines = Vec::new();
    let mut margins = Vec::new();
    let mut cursor = 0usize;

    while cursor <= body.len() {
        let line_end = body[cursor..]
            .find('\n')
            .map(|index| cursor + index)
            .unwrap_or(body.len());
        let raw_line = &body[cursor..line_end];
        let raw_start = body_start + cursor;

        // Strip the visual JSDoc margin: optional indentation, leading `*`,
        // then at most one separator space. Capture each part as a slice.
        let mut pos = 0usize;
        let bytes = raw_line.as_bytes();

        // Capture initial (indentation before *)
        let initial_start = pos;
        while pos < bytes.len() && matches!(bytes[pos], b' ' | b'\t') {
            pos += 1;
        }
        let initial = &raw_line[initial_start..pos];

        // Capture delimiter (* itself)
        let delimiter_start = pos;
        if bytes.get(pos) == Some(&b'*') {
            pos += 1;
        }
        let delimiter = &raw_line[delimiter_start..pos];

        // Capture post_delimiter (at most one space/tab after *)
        let post_delim_start = pos;
        if !delimiter.is_empty() && matches!(bytes.get(pos), Some(b' ' | b'\t')) {
            pos += 1;
        }
        let post_delimiter = &raw_line[post_delim_start..pos];

        let content_start = pos;
        let content = &raw_line[content_start..];

        // Pre-compute emptiness once (avoids repeated trim() calls downstream).
        let is_content_empty = content.bytes().all(|b| b == b' ' || b == b'\t');

        // Capture line_end (newline characters)
        let line_end_str = if line_end < body.len() {
            if line_end > 0 && body.as_bytes().get(line_end - 1) == Some(&b'\r') {
                "\r\n"
            } else {
                "\n"
            }
        } else {
            ""
        };

        let absolute_content_start =
            base_offset + u32::try_from(raw_start + content_start).unwrap();
        let absolute_content_end = base_offset + u32::try_from(raw_start + raw_line.len()).unwrap();

        lines.push(LogicalLine {
            content,
            content_start: absolute_content_start,
            content_end: absolute_content_end,
        });
        margins.push(MarginInfo {
            initial,
            delimiter,
            post_delimiter,
            line_end: line_end_str,
            is_content_empty,
        });

        if line_end == body.len() {
            break;
        }
        cursor = line_end + 1;
    }

    ScanResult { lines, margins }
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
        let result = logical_lines(source, 100);

        assert_eq!(result.lines.len(), 4);
        assert_eq!(result.lines[0].content, "");
        assert_eq!(result.lines[0].content_start, 103);
        assert_eq!(result.lines[0].content_end, 103);

        assert_eq!(result.lines[1].content, "Find a user.");
        assert_eq!(result.lines[1].content_start, 107);
        assert_eq!(result.lines[1].content_end, 119);

        assert_eq!(result.lines[2].content, "@param {string} id");
        assert_eq!(result.lines[2].content_start, 123);
        assert_eq!(result.lines[2].content_end, 141);

        assert_eq!(result.lines[3].content, "");
        assert_eq!(result.lines[3].content_start, 143);
        assert_eq!(result.lines[3].content_end, 143);

        // Verify margins are captured
        assert_eq!(result.margins[1].delimiter, "*");
        assert_eq!(result.margins[1].post_delimiter, " ");
        assert!(result.margins[0].is_content_empty);
        assert!(!result.margins[1].is_content_empty);
    }

    #[test]
    fn preserves_trailing_body_text() {
        let source = "/** value   */";
        let result = logical_lines(source, 0);

        assert_eq!(result.lines.len(), 1);
        assert_eq!(result.lines[0].content, "value   ");
        assert_eq!(result.lines[0].content_start, 4);
        assert_eq!(result.lines[0].content_end, 12);
    }

    #[test]
    fn returns_no_lines_for_invalid_block_shells() {
        assert!(logical_lines("/* plain */", 0).lines.is_empty());
        assert!(logical_lines("/** unclosed", 0).lines.is_empty());
    }
}
