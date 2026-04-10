// @author kazuya kawaguchi (a.k.a. kazupon)
// @license MIT
//

//! JSON serializer for the ESTree-like JS-facing shape.

use crate::analyzer::AnalysisOutput;
use crate::ast::{
    JsdocBlock, JsdocDescriptionLine, JsdocGenericTagBody, JsdocInlineTag, JsdocInlineTagFormat,
    JsdocTag, JsdocTagBody, JsdocTagValue, JsdocTypeLine,
};
use crate::validator::ValidationOutput;

/// Serialize a parsed comment and optional derived outputs to JSON.
///
/// The Rust AST uses concrete node types and `Span`. The JS-facing shape emits
/// ESTree-like `type`, `start`, `end`, and `range` fields.
pub fn serialize_comment_json(
    comment: &JsdocBlock<'_>,
    validation: Option<&ValidationOutput>,
    analysis: Option<&AnalysisOutput<'_>>,
) -> String {
    let mut json = String::new();
    push_block(&mut json, comment);

    if validation.is_none() && analysis.is_none() {
        return json;
    }

    // Wrap only when derived outputs are requested, keeping parser-only output
    // close to the AST root shape.
    let mut wrapped = String::new();
    wrapped.push('{');
    push_key(&mut wrapped, "comment");
    wrapped.push_str(&json);

    if let Some(validation) = validation {
        wrapped.push(',');
        push_key(&mut wrapped, "validation");
        wrapped.push('{');
        push_key(&mut wrapped, "diagnosticCount");
        push_usize(&mut wrapped, validation.diagnostics.len());
        wrapped.push('}');
    }

    if let Some(analysis) = analysis {
        wrapped.push(',');
        push_key(&mut wrapped, "analysis");
        wrapped.push('{');
        push_key(&mut wrapped, "tagCount");
        push_usize(&mut wrapped, analysis.tag_count);
        wrapped.push(',');
        push_key(&mut wrapped, "hasInlineTags");
        wrapped.push_str(if analysis.has_inline_tags {
            "true"
        } else {
            "false"
        });
        wrapped.push(',');
        push_key(&mut wrapped, "tagNames");
        push_string_array(&mut wrapped, &analysis.tag_names);
        wrapped.push(',');
        push_key(&mut wrapped, "parameterNames");
        push_string_array(&mut wrapped, &analysis.parameter_names);
        wrapped.push(',');
        push_key(&mut wrapped, "customTagNames");
        push_string_array(&mut wrapped, &analysis.custom_tag_names);
        wrapped.push('}');
    }

    wrapped.push('}');
    wrapped
}

fn push_block(json: &mut String, block: &JsdocBlock<'_>) {
    json.push('{');
    push_node_header(json, "JsdocBlock", block.span.start, block.span.end);
    json.push(',');
    push_key(json, "description");
    push_string(json, block.description.unwrap_or_default());
    json.push(',');
    push_key(json, "descriptionLines");
    push_description_lines(json, &block.description_lines);
    json.push(',');
    push_key(json, "tags");
    json.push('[');
    for (index, tag) in block.tags.iter().enumerate() {
        if index > 0 {
            json.push(',');
        }
        push_tag(json, tag);
    }
    json.push(']');
    json.push(',');
    push_key(json, "inlineTags");
    push_inline_tags(json, &block.inline_tags);
    json.push('}');
}

fn push_description_lines(json: &mut String, lines: &[JsdocDescriptionLine<'_>]) {
    json.push('[');
    for (index, line) in lines.iter().enumerate() {
        if index > 0 {
            json.push(',');
        }
        push_description_line(json, line);
    }
    json.push(']');
}

fn push_description_line(json: &mut String, line: &JsdocDescriptionLine<'_>) {
    json.push('{');
    push_node_header(json, "JsdocDescriptionLine", line.span.start, line.span.end);
    json.push(',');
    push_key(json, "delimiter");
    push_string(json, line.delimiter);
    json.push(',');
    push_key(json, "postDelimiter");
    push_string(json, line.post_delimiter);
    json.push(',');
    push_key(json, "initial");
    push_string(json, line.initial);
    json.push(',');
    push_key(json, "description");
    push_string(json, line.description);
    json.push('}');
}

fn push_tag(json: &mut String, tag: &JsdocTag<'_>) {
    json.push('{');
    push_node_header(json, "JsdocTag", tag.span.start, tag.span.end);
    json.push(',');
    push_key(json, "tag");
    push_string(json, tag.tag.value);
    json.push(',');
    push_key(json, "rawType");
    match tag.raw_type {
        Some(raw_type) => push_string(json, raw_type.raw),
        None => json.push_str("null"),
    }
    json.push(',');
    push_key(json, "parsedType");
    json.push_str("null");
    json.push(',');
    push_key(json, "name");
    match tag.name {
        Some(name) => push_string(json, name.raw),
        None => json.push_str("null"),
    }
    json.push(',');
    push_key(json, "optional");
    json.push_str(if tag.optional { "true" } else { "false" });
    json.push(',');
    push_key(json, "defaultValue");
    match tag.default_value {
        Some(value) => push_string(json, value),
        None => json.push_str("null"),
    }
    json.push(',');
    push_key(json, "description");
    push_string(json, tag.description.unwrap_or_default());
    json.push(',');
    push_key(json, "rawBody");
    match tag.raw_body {
        Some(raw_body) => push_string(json, raw_body),
        None => json.push_str("null"),
    }
    json.push(',');
    push_key(json, "typeLines");
    push_type_lines(json, &tag.type_lines);
    json.push(',');
    push_key(json, "descriptionLines");
    push_description_lines(json, &tag.description_lines);
    json.push(',');
    push_key(json, "inlineTags");
    push_inline_tags(json, &tag.inline_tags);
    json.push(',');
    push_key(json, "body");
    match tag.body.as_ref() {
        Some(body) => push_tag_body(json, body.as_ref()),
        None => json.push_str("null"),
    }
    json.push('}');
}

fn push_type_lines(json: &mut String, lines: &[JsdocTypeLine<'_>]) {
    json.push('[');
    for (index, line) in lines.iter().enumerate() {
        if index > 0 {
            json.push(',');
        }
        json.push('{');
        push_node_header(json, "JsdocTypeLine", line.span.start, line.span.end);
        json.push(',');
        push_key(json, "delimiter");
        push_string(json, line.delimiter);
        json.push(',');
        push_key(json, "postDelimiter");
        push_string(json, line.post_delimiter);
        json.push(',');
        push_key(json, "initial");
        push_string(json, line.initial);
        json.push(',');
        push_key(json, "rawType");
        push_string(json, line.raw_type);
        json.push('}');
    }
    json.push(']');
}

fn push_inline_tags(json: &mut String, tags: &[JsdocInlineTag<'_>]) {
    json.push('[');
    for (index, tag) in tags.iter().enumerate() {
        if index > 0 {
            json.push(',');
        }
        push_inline_tag(json, tag);
    }
    json.push(']');
}

fn push_inline_tag(json: &mut String, tag: &JsdocInlineTag<'_>) {
    json.push('{');
    push_node_header(json, "JsdocInlineTag", tag.span.start, tag.span.end);
    json.push(',');
    push_key(json, "tag");
    push_string(json, tag.tag.value);
    json.push(',');
    push_key(json, "namepathOrURL");
    match tag.namepath_or_url {
        Some(value) => push_string(json, value),
        None => json.push_str("null"),
    }
    json.push(',');
    push_key(json, "text");
    match tag.text {
        Some(value) => push_string(json, value),
        None => json.push_str("null"),
    }
    json.push(',');
    push_key(json, "format");
    push_string(json, inline_tag_format(tag.format));
    json.push(',');
    push_key(json, "rawBody");
    match tag.raw_body {
        Some(value) => push_string(json, value),
        None => json.push_str("null"),
    }
    json.push('}');
}

fn push_tag_body(json: &mut String, body: &JsdocTagBody<'_>) {
    match body {
        JsdocTagBody::Generic(body) => push_generic_tag_body(json, body),
        JsdocTagBody::Borrows(body) => {
            json.push('{');
            push_key(json, "kind");
            push_string(json, "borrows");
            json.push(',');
            push_key(json, "source");
            push_tag_value(json, &body.source);
            json.push(',');
            push_key(json, "target");
            push_tag_value(json, &body.target);
            json.push('}');
        }
        JsdocTagBody::Raw(body) => {
            json.push('{');
            push_key(json, "kind");
            push_string(json, "raw");
            json.push(',');
            push_key(json, "raw");
            push_string(json, body.raw);
            json.push('}');
        }
    }
}

fn push_generic_tag_body(json: &mut String, body: &JsdocGenericTagBody<'_>) {
    json.push('{');
    push_key(json, "kind");
    push_string(json, "generic");
    json.push(',');
    push_key(json, "typeSource");
    match body.type_source {
        Some(type_source) => push_string(json, type_source.raw),
        None => json.push_str("null"),
    }
    json.push(',');
    push_key(json, "value");
    match body.value.as_ref() {
        Some(value) => push_tag_value(json, value),
        None => json.push_str("null"),
    }
    json.push(',');
    push_key(json, "separator");
    match body.separator {
        Some(_) => push_string(json, "-"),
        None => json.push_str("null"),
    }
    json.push(',');
    push_key(json, "description");
    match body.description {
        Some(description) => push_string(json, description),
        None => json.push_str("null"),
    }
    json.push('}');
}

fn push_tag_value(json: &mut String, value: &JsdocTagValue<'_>) {
    json.push('{');
    match value {
        JsdocTagValue::Parameter(parameter) => {
            push_key(json, "kind");
            push_string(json, "parameter");
            json.push(',');
            push_key(json, "path");
            push_string(json, parameter.path);
            json.push(',');
            push_key(json, "optional");
            json.push_str(if parameter.optional { "true" } else { "false" });
            json.push(',');
            push_key(json, "defaultValue");
            match parameter.default_value {
                Some(value) => push_string(json, value),
                None => json.push_str("null"),
            }
        }
        JsdocTagValue::Namepath(namepath) => {
            push_key(json, "kind");
            push_string(json, "namepath");
            json.push(',');
            push_key(json, "raw");
            push_string(json, namepath.raw);
        }
        JsdocTagValue::Identifier(identifier) => {
            push_key(json, "kind");
            push_string(json, "identifier");
            json.push(',');
            push_key(json, "name");
            push_string(json, identifier.name);
        }
        JsdocTagValue::Raw(text) => {
            push_key(json, "kind");
            push_string(json, "raw");
            json.push(',');
            push_key(json, "value");
            push_string(json, text.value);
        }
    }
    json.push('}');
}

fn push_node_header(json: &mut String, node_type: &str, start: u32, end: u32) {
    push_key(json, "type");
    push_string(json, node_type);
    json.push(',');
    push_key(json, "start");
    push_u32(json, start);
    json.push(',');
    push_key(json, "end");
    push_u32(json, end);
    json.push(',');
    push_key(json, "range");
    json.push('[');
    push_u32(json, start);
    json.push(',');
    push_u32(json, end);
    json.push(']');
}

fn inline_tag_format(format: JsdocInlineTagFormat) -> &'static str {
    match format {
        JsdocInlineTagFormat::Plain => "plain",
        JsdocInlineTagFormat::Pipe => "pipe",
        JsdocInlineTagFormat::Space => "space",
        JsdocInlineTagFormat::Prefix => "prefix",
        JsdocInlineTagFormat::Unknown => "unknown",
    }
}

fn push_key(json: &mut String, key: &str) {
    push_string(json, key);
    json.push(':');
}

fn push_u32(json: &mut String, value: u32) {
    json.push_str(&value.to_string());
}

fn push_usize(json: &mut String, value: usize) {
    json.push_str(&value.to_string());
}

fn push_string_array(json: &mut String, values: &[&str]) {
    json.push('[');
    for (index, value) in values.iter().enumerate() {
        if index > 0 {
            json.push(',');
        }
        push_string(json, value);
    }
    json.push(']');
}

fn push_string(json: &mut String, value: &str) {
    json.push('"');
    for ch in value.chars() {
        match ch {
            '"' => json.push_str("\\\""),
            '\\' => json.push_str("\\\\"),
            '\n' => json.push_str("\\n"),
            '\r' => json.push_str("\\r"),
            '\t' => json.push_str("\\t"),
            _ => json.push(ch),
        }
    }
    json.push('"');
}

#[cfg(test)]
mod tests {
    use oxc_allocator::Allocator;

    use crate::analyzer::analyze_comment;
    use crate::parse_comment;
    use crate::parser::ParseOptions;
    use crate::validator::{ValidationOptions, validate_comment};

    use super::serialize_comment_json;

    #[test]
    fn serializes_comment_with_validation_and_analysis() {
        let allocator = Allocator::default();
        let parsed = parse_comment(
            &allocator,
            "/** See {@link UserService}.\n * @param {string} id - The user ID\n */",
            10,
            ParseOptions::default(),
        );
        let comment = parsed.comment.expect("expected comment");
        let validation = validate_comment(&comment, ValidationOptions::default());
        let analysis = analyze_comment(&comment);

        let json = serialize_comment_json(&comment, Some(&validation), Some(&analysis));

        assert!(json.contains("\"type\":\"JsdocBlock\""));
        assert!(json.contains("\"tag\":\"param\""));
        assert!(json.contains("\"type\":\"JsdocInlineTag\""));
        assert!(json.contains("\"diagnosticCount\":0"));
        assert!(json.contains("\"parameterNames\":[\"id\"]"));
        assert!(json.contains("\"start\":10"));
    }
}
