// @author kazuya kawaguchi (a.k.a. kazupon)
// @license MIT
//

#[derive(Debug, Clone, Copy)]
pub struct LogicalLine<'a> {
    pub content: &'a str,
    pub content_start: u32,
    pub content_end: u32,
}

pub fn is_jsdoc_block(source_text: &str) -> bool {
    source_text.starts_with("/**")
}

pub fn has_closing_block(source_text: &str) -> bool {
    source_text.ends_with("*/")
}

pub fn body_range(source_text: &str) -> Option<(usize, usize)> {
    if !is_jsdoc_block(source_text) || !has_closing_block(source_text) || source_text.len() < 5 {
        return None;
    }

    Some((3, source_text.len() - 2))
}

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
