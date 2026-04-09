// @author kazuya kawaguchi (a.k.a. kazupon)
// @license MIT
//

//! JSON serializer for the initial JS-facing shape.

use crate::analyzer::AnalysisOutput;
use crate::ast::{
    BlockTag, BlockTagBody, Description, DescriptionPart, InlineTag, JSDocComment, TagValueToken,
    Text,
};
use crate::validator::ValidationOutput;

pub fn serialize_comment_json(
    comment: &JSDocComment<'_>,
    validation: Option<&ValidationOutput>,
    analysis: Option<&AnalysisOutput<'_>>,
) -> String {
    let mut json = String::new();
    json.push('{');
    push_key(&mut json, "span");
    push_span(&mut json, comment.span.start, comment.span.end);
    json.push(',');
    push_key(&mut json, "description");
    match comment.description.as_ref() {
        Some(description) => push_description(&mut json, description),
        None => json.push_str("null"),
    }
    json.push(',');
    push_key(&mut json, "tags");
    json.push('[');
    for (index, tag) in comment.tags.iter().enumerate() {
        if index > 0 {
            json.push(',');
        }
        push_block_tag(&mut json, tag);
    }
    json.push(']');

    if let Some(validation) = validation {
        json.push(',');
        push_key(&mut json, "validation");
        json.push('{');
        push_key(&mut json, "diagnosticCount");
        json.push_str(&validation.diagnostics.len().to_string());
        json.push('}');
    }

    if let Some(analysis) = analysis {
        json.push(',');
        push_key(&mut json, "analysis");
        json.push('{');
        push_key(&mut json, "tagCount");
        json.push_str(&analysis.tag_count.to_string());
        json.push(',');
        push_key(&mut json, "hasInlineTags");
        json.push_str(if analysis.has_inline_tags {
            "true"
        } else {
            "false"
        });
        json.push(',');
        push_key(&mut json, "tagNames");
        push_string_array(&mut json, &analysis.tag_names);
        json.push(',');
        push_key(&mut json, "parameterNames");
        push_string_array(&mut json, &analysis.parameter_names);
        json.push(',');
        push_key(&mut json, "customTagNames");
        push_string_array(&mut json, &analysis.custom_tag_names);
        json.push('}');
    }

    json.push('}');
    json
}

fn push_description(json: &mut String, description: &Description<'_>) {
    json.push('{');
    push_key(json, "span");
    push_span(json, description.span.start, description.span.end);
    json.push(',');
    push_key(json, "parts");
    json.push('[');
    for (index, part) in description.parts.iter().enumerate() {
        if index > 0 {
            json.push(',');
        }
        match part {
            DescriptionPart::Text(text) => push_text_part(json, text),
            DescriptionPart::InlineTag(tag) => push_inline_tag_part(json, tag),
        }
    }
    json.push(']');
    json.push('}');
}

fn push_text_part(json: &mut String, text: &Text<'_>) {
    json.push('{');
    push_key(json, "kind");
    push_string(json, "text");
    json.push(',');
    push_key(json, "value");
    push_string(json, text.value);
    json.push(',');
    push_key(json, "span");
    push_span(json, text.span.start, text.span.end);
    json.push('}');
}

fn push_inline_tag_part(json: &mut String, tag: &InlineTag<'_>) {
    json.push('{');
    push_key(json, "kind");
    push_string(json, "inlineTag");
    json.push(',');
    push_key(json, "tagName");
    push_string(json, tag.tag_name.value);
    json.push(',');
    push_key(json, "span");
    push_span(json, tag.span.start, tag.span.end);
    json.push(',');
    push_key(json, "body");
    match tag.body.as_ref() {
        Some(body) => push_string(json, body.raw),
        None => json.push_str("null"),
    }
    json.push('}');
}

fn push_block_tag(json: &mut String, tag: &BlockTag<'_>) {
    json.push('{');
    push_key(json, "tagName");
    push_string(json, tag.tag_name.value);
    json.push(',');
    push_key(json, "span");
    push_span(json, tag.span.start, tag.span.end);
    json.push(',');
    push_key(json, "rawBody");
    match tag.raw_body.as_ref() {
        Some(raw) => push_string(json, raw.value),
        None => json.push_str("null"),
    }
    json.push(',');
    push_key(json, "body");
    match tag.body.as_ref() {
        Some(body) => match body.as_ref() {
            BlockTagBody::Generic(body) => {
                json.push('{');
                push_key(json, "kind");
                push_string(json, "generic");
                json.push(',');
                push_key(json, "typeExpression");
                match body.type_expression.as_ref() {
                    Some(type_expression) => push_string(json, type_expression.raw),
                    None => json.push_str("null"),
                }
                json.push(',');
                push_key(json, "value");
                match body.value.as_ref() {
                    Some(value) => push_tag_value(json, value.as_ref()),
                    None => json.push_str("null"),
                }
                json.push(',');
                push_key(json, "description");
                match body.description.as_ref() {
                    Some(description) => push_description(json, description),
                    None => json.push_str("null"),
                }
                json.push('}');
            }
            BlockTagBody::Borrows(body) => {
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
        },
        None => json.push_str("null"),
    }
    json.push('}');
}

fn push_tag_value(json: &mut String, value: &TagValueToken<'_>) {
    json.push('{');
    match value {
        TagValueToken::Raw(text) => {
            push_key(json, "kind");
            push_string(json, "raw");
            json.push(',');
            push_key(json, "value");
            push_string(json, text.value);
        }
        TagValueToken::Parameter(parameter) => {
            push_key(json, "kind");
            push_string(json, "parameter");
            json.push(',');
            push_key(json, "path");
            push_string(json, parameter.path.raw);
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
        TagValueToken::NamePath(namepath) => {
            push_key(json, "kind");
            push_string(json, "namePath");
            json.push(',');
            push_key(json, "value");
            push_string(json, namepath.raw);
        }
    }
    json.push('}');
}

fn push_key(json: &mut String, key: &str) {
    push_string(json, key);
    json.push(':');
}

fn push_span(json: &mut String, start: u32, end: u32) {
    json.push('{');
    push_key(json, "start");
    json.push_str(&start.to_string());
    json.push(',');
    push_key(json, "end");
    json.push_str(&end.to_string());
    json.push('}');
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

        assert!(json.contains("\"tagName\":\"param\""));
        assert!(json.contains("\"kind\":\"inlineTag\""));
        assert!(json.contains("\"diagnosticCount\":0"));
        assert!(json.contains("\"parameterNames\":[\"id\"]"));
        assert!(json.contains("\"start\":10"));
    }
}
