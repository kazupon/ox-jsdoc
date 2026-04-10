// @author kazuya kawaguchi (a.k.a. kazupon)
// @license MIT
//

//! JSON serializer for the ESTree-like JS-facing shape.
//!
//! Uses `serde::Serialize` + `serde_json::to_string` for efficient JSON
//! generation. The intermediate `Ser*` structs map the arena-allocated AST
//! into a serializable form without copying source text.

use serde::Serialize;

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
    if validation.is_none() && analysis.is_none() {
        let block = SerBlock::from(comment);
        return serde_json::to_string(&block).unwrap_or_default();
    }

    let wrapped = SerWrapped {
        comment: SerBlock::from(comment),
        validation: validation.map(SerValidation::from),
        analysis: analysis.map(SerAnalysis::from),
    };
    serde_json::to_string(&wrapped).unwrap_or_default()
}

// ---------------------------------------------------------------------------
// Wrapped output (when validation/analysis are requested)
// ---------------------------------------------------------------------------

#[derive(Serialize)]
struct SerWrapped<'a> {
    comment: SerBlock<'a>,
    #[serde(skip_serializing_if = "Option::is_none")]
    validation: Option<SerValidation>,
    #[serde(skip_serializing_if = "Option::is_none")]
    analysis: Option<SerAnalysis<'a>>,
}

#[derive(Serialize)]
struct SerValidation {
    #[serde(rename = "diagnosticCount")]
    diagnostic_count: usize,
}

impl From<&ValidationOutput> for SerValidation {
    fn from(v: &ValidationOutput) -> Self {
        Self {
            diagnostic_count: v.diagnostics.len(),
        }
    }
}

#[derive(Serialize)]
struct SerAnalysis<'a> {
    #[serde(rename = "tagCount")]
    tag_count: usize,
    #[serde(rename = "hasInlineTags")]
    has_inline_tags: bool,
    #[serde(rename = "tagNames")]
    tag_names: &'a [&'a str],
    #[serde(rename = "parameterNames")]
    parameter_names: &'a [&'a str],
    #[serde(rename = "customTagNames")]
    custom_tag_names: &'a [&'a str],
}

impl<'a> From<&'a AnalysisOutput<'a>> for SerAnalysis<'a> {
    fn from(a: &'a AnalysisOutput<'a>) -> Self {
        Self {
            tag_count: a.tag_count,
            has_inline_tags: a.has_inline_tags,
            tag_names: &a.tag_names,
            parameter_names: &a.parameter_names,
            custom_tag_names: &a.custom_tag_names,
        }
    }
}

// ---------------------------------------------------------------------------
// AST node serialization structs
// ---------------------------------------------------------------------------

#[derive(Serialize)]
struct SerBlock<'a> {
    #[serde(rename = "type")]
    node_type: &'static str,
    start: u32,
    end: u32,
    range: [u32; 2],
    description: &'a str,
    #[serde(rename = "descriptionLines")]
    description_lines: Vec<SerDescriptionLine<'a>>,
    tags: Vec<SerTag<'a>>,
    #[serde(rename = "inlineTags")]
    inline_tags: Vec<SerInlineTag<'a>>,
}

impl<'a> From<&'a JsdocBlock<'a>> for SerBlock<'a> {
    fn from(block: &'a JsdocBlock<'a>) -> Self {
        Self {
            node_type: "JsdocBlock",
            start: block.span.start,
            end: block.span.end,
            range: [block.span.start, block.span.end],
            description: block.description.unwrap_or_default(),
            description_lines: block
                .description_lines
                .iter()
                .map(SerDescriptionLine::from)
                .collect(),
            tags: block.tags.iter().map(SerTag::from).collect(),
            inline_tags: block
                .inline_tags
                .iter()
                .map(SerInlineTag::from)
                .collect(),
        }
    }
}

#[derive(Serialize)]
struct SerDescriptionLine<'a> {
    #[serde(rename = "type")]
    node_type: &'static str,
    start: u32,
    end: u32,
    range: [u32; 2],
    delimiter: &'a str,
    #[serde(rename = "postDelimiter")]
    post_delimiter: &'a str,
    initial: &'a str,
    description: &'a str,
}

impl<'a> From<&'a JsdocDescriptionLine<'a>> for SerDescriptionLine<'a> {
    fn from(line: &'a JsdocDescriptionLine<'a>) -> Self {
        Self {
            node_type: "JsdocDescriptionLine",
            start: line.span.start,
            end: line.span.end,
            range: [line.span.start, line.span.end],
            delimiter: line.delimiter,
            post_delimiter: line.post_delimiter,
            initial: line.initial,
            description: line.description,
        }
    }
}

#[derive(Serialize)]
struct SerTag<'a> {
    #[serde(rename = "type")]
    node_type: &'static str,
    start: u32,
    end: u32,
    range: [u32; 2],
    tag: &'a str,
    #[serde(rename = "rawType")]
    raw_type: Option<&'a str>,
    #[serde(rename = "parsedType")]
    parsed_type: Option<()>,
    name: Option<&'a str>,
    optional: bool,
    #[serde(rename = "defaultValue")]
    default_value: Option<&'a str>,
    description: &'a str,
    #[serde(rename = "rawBody")]
    raw_body: Option<&'a str>,
    #[serde(rename = "typeLines")]
    type_lines: Vec<SerTypeLine<'a>>,
    #[serde(rename = "descriptionLines")]
    description_lines: Vec<SerDescriptionLine<'a>>,
    #[serde(rename = "inlineTags")]
    inline_tags: Vec<SerInlineTag<'a>>,
    body: Option<SerTagBody<'a>>,
}

impl<'a> From<&'a JsdocTag<'a>> for SerTag<'a> {
    fn from(tag: &'a JsdocTag<'a>) -> Self {
        Self {
            node_type: "JsdocTag",
            start: tag.span.start,
            end: tag.span.end,
            range: [tag.span.start, tag.span.end],
            tag: tag.tag.value,
            raw_type: tag.raw_type.map(|rt| rt.raw),
            parsed_type: None,
            name: tag.name.map(|n| n.raw),
            optional: tag.optional,
            default_value: tag.default_value,
            description: tag.description.unwrap_or_default(),
            raw_body: tag.raw_body,
            type_lines: tag.type_lines.iter().map(SerTypeLine::from).collect(),
            description_lines: tag
                .description_lines
                .iter()
                .map(SerDescriptionLine::from)
                .collect(),
            inline_tags: tag
                .inline_tags
                .iter()
                .map(SerInlineTag::from)
                .collect(),
            body: tag.body.as_ref().map(|b| SerTagBody::from(b.as_ref())),
        }
    }
}

#[derive(Serialize)]
struct SerTypeLine<'a> {
    #[serde(rename = "type")]
    node_type: &'static str,
    start: u32,
    end: u32,
    range: [u32; 2],
    delimiter: &'a str,
    #[serde(rename = "postDelimiter")]
    post_delimiter: &'a str,
    initial: &'a str,
    #[serde(rename = "rawType")]
    raw_type: &'a str,
}

impl<'a> From<&'a JsdocTypeLine<'a>> for SerTypeLine<'a> {
    fn from(line: &'a JsdocTypeLine<'a>) -> Self {
        Self {
            node_type: "JsdocTypeLine",
            start: line.span.start,
            end: line.span.end,
            range: [line.span.start, line.span.end],
            delimiter: line.delimiter,
            post_delimiter: line.post_delimiter,
            initial: line.initial,
            raw_type: line.raw_type,
        }
    }
}

#[derive(Serialize)]
struct SerInlineTag<'a> {
    #[serde(rename = "type")]
    node_type: &'static str,
    start: u32,
    end: u32,
    range: [u32; 2],
    tag: &'a str,
    #[serde(rename = "namepathOrURL")]
    namepath_or_url: Option<&'a str>,
    text: Option<&'a str>,
    format: &'static str,
    #[serde(rename = "rawBody")]
    raw_body: Option<&'a str>,
}

impl<'a> From<&'a JsdocInlineTag<'a>> for SerInlineTag<'a> {
    fn from(tag: &'a JsdocInlineTag<'a>) -> Self {
        Self {
            node_type: "JsdocInlineTag",
            start: tag.span.start,
            end: tag.span.end,
            range: [tag.span.start, tag.span.end],
            tag: tag.tag.value,
            namepath_or_url: tag.namepath_or_url,
            text: tag.text,
            format: match tag.format {
                JsdocInlineTagFormat::Plain => "plain",
                JsdocInlineTagFormat::Pipe => "pipe",
                JsdocInlineTagFormat::Space => "space",
                JsdocInlineTagFormat::Prefix => "prefix",
                JsdocInlineTagFormat::Unknown => "unknown",
            },
            raw_body: tag.raw_body,
        }
    }
}

#[derive(Serialize)]
#[serde(tag = "kind")]
enum SerTagBody<'a> {
    #[serde(rename = "generic")]
    Generic(SerGenericTagBody<'a>),
    #[serde(rename = "borrows")]
    Borrows {
        source: SerTagValue<'a>,
        target: SerTagValue<'a>,
    },
    #[serde(rename = "raw")]
    Raw { raw: &'a str },
}

impl<'a> From<&'a JsdocTagBody<'a>> for SerTagBody<'a> {
    fn from(body: &'a JsdocTagBody<'a>) -> Self {
        match body {
            JsdocTagBody::Generic(g) => SerTagBody::Generic(SerGenericTagBody::from(g.as_ref())),
            JsdocTagBody::Borrows(b) => SerTagBody::Borrows {
                source: SerTagValue::from(&b.source),
                target: SerTagValue::from(&b.target),
            },
            JsdocTagBody::Raw(r) => SerTagBody::Raw { raw: r.raw },
        }
    }
}

#[derive(Serialize)]
struct SerGenericTagBody<'a> {
    #[serde(rename = "typeSource")]
    type_source: Option<&'a str>,
    value: Option<SerTagValue<'a>>,
    separator: Option<&'static str>,
    description: Option<&'a str>,
}

impl<'a> From<&'a JsdocGenericTagBody<'a>> for SerGenericTagBody<'a> {
    fn from(body: &'a JsdocGenericTagBody<'a>) -> Self {
        Self {
            type_source: body.type_source.map(|ts| ts.raw),
            value: body.value.as_ref().map(SerTagValue::from),
            separator: body.separator.map(|_| "-"),
            description: body.description,
        }
    }
}

#[derive(Serialize)]
#[serde(tag = "kind")]
enum SerTagValue<'a> {
    #[serde(rename = "parameter")]
    Parameter {
        path: &'a str,
        optional: bool,
        #[serde(rename = "defaultValue")]
        default_value: Option<&'a str>,
    },
    #[serde(rename = "namepath")]
    Namepath { raw: &'a str },
    #[serde(rename = "identifier")]
    Identifier { name: &'a str },
    #[serde(rename = "raw")]
    Raw { value: &'a str },
}

impl<'a> From<&'a JsdocTagValue<'a>> for SerTagValue<'a> {
    fn from(value: &'a JsdocTagValue<'a>) -> Self {
        match value {
            JsdocTagValue::Parameter(p) => SerTagValue::Parameter {
                path: p.path,
                optional: p.optional,
                default_value: p.default_value,
            },
            JsdocTagValue::Namepath(n) => SerTagValue::Namepath { raw: n.raw },
            JsdocTagValue::Identifier(i) => SerTagValue::Identifier { name: i.name },
            JsdocTagValue::Raw(t) => SerTagValue::Raw { value: t.value },
        }
    }
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

    #[test]
    fn serializes_comment_without_derived_outputs() {
        let allocator = Allocator::default();
        let parsed = parse_comment(
            &allocator,
            "/** @param {string} id */",
            0,
            ParseOptions::default(),
        );
        let comment = parsed.comment.expect("expected comment");

        let json = serialize_comment_json(&comment, None, None);

        assert!(json.contains("\"type\":\"JsdocBlock\""));
        assert!(json.contains("\"tag\":\"param\""));
        // Should NOT be wrapped
        assert!(!json.contains("\"comment\":{"));
    }
}
