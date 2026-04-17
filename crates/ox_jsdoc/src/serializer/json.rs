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
    JsdocTag, JsdocTagBody, JsdocTagValue, JsdocType, JsdocTypeLine,
};
use crate::type_parser::ast::*;
use crate::validator::ValidationOutput;

// ---------------------------------------------------------------------------
// SerializeOptions
// ---------------------------------------------------------------------------

/// Controls how the AST is serialized to JSON.
#[derive(Debug, Clone, Copy)]
pub struct SerializeOptions {
    /// Output jsdoccomment-compatible fields (delimiter, postDelimiter, initial, etc.)
    /// and exclude ox-jsdoc-specific fields (optional, defaultValue, rawBody, body).
    pub compat_mode: bool,
    /// Convert `None` optional fields to `""` instead of `null` / omitting.
    pub empty_string_for_null: bool,
    /// Include ESTree position fields (start, end, range). Default: true.
    pub include_positions: bool,
    /// Spacing mode for compat_mode. Only effective when compat_mode is true.
    pub spacing: SpacingMode,
}

/// Spacing mode matching jsdoccomment's `commentParserToESTree()` spacing option.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpacingMode {
    /// Filter empty description lines (jsdoccomment default).
    Compact,
    /// Preserve all description lines including empty ones.
    Preserve,
}

impl Default for SerializeOptions {
    fn default() -> Self {
        Self {
            compat_mode: false,
            empty_string_for_null: false,
            include_positions: true,
            spacing: SpacingMode::Compact,
        }
    }
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Serialize a parsed comment and optional derived outputs to JSON.
///
/// The Rust AST uses concrete node types and `Span`. The JS-facing shape emits
/// ESTree-like `type`, `start`, `end`, and `range` fields.
pub fn serialize_comment_json(
    comment: &JsdocBlock<'_>,
    validation: Option<&ValidationOutput>,
    analysis: Option<&AnalysisOutput<'_>>,
) -> String {
    serialize_comment_json_with_options(comment, validation, analysis, &SerializeOptions::default())
}

/// Serialize with explicit options controlling compat_mode and field output.
pub fn serialize_comment_json_with_options(
    comment: &JsdocBlock<'_>,
    validation: Option<&ValidationOutput>,
    analysis: Option<&AnalysisOutput<'_>>,
    options: &SerializeOptions,
) -> String {
    if validation.is_none() && analysis.is_none() {
        let block = SerBlock::new(comment, options);
        return serde_json::to_string(&block).unwrap_or_default();
    }

    let wrapped = SerWrapped {
        comment: SerBlock::new(comment, options),
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

    // Position fields (conditionally included)
    #[serde(skip_serializing_if = "Option::is_none")]
    start: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    end: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    range: Option<[u32; 2]>,

    // compat_mode fields
    #[serde(skip_serializing_if = "Option::is_none")]
    delimiter: Option<&'a str>,
    #[serde(rename = "postDelimiter", skip_serializing_if = "Option::is_none")]
    post_delimiter: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    initial: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    terminal: Option<&'a str>,
    #[serde(rename = "lineEnd", skip_serializing_if = "Option::is_none")]
    line_end: Option<&'a str>,
    #[serde(rename = "delimiterLineBreak", skip_serializing_if = "Option::is_none")]
    delimiter_line_break: Option<&'a str>,
    #[serde(
        rename = "preterminalLineBreak",
        skip_serializing_if = "Option::is_none"
    )]
    preterminal_line_break: Option<&'a str>,

    // compat_mode line metadata
    #[serde(rename = "endLine", skip_serializing_if = "Option::is_none")]
    end_line: Option<u32>,
    #[serde(
        rename = "descriptionStartLine",
        skip_serializing_if = "Option::is_none"
    )]
    description_start_line: Option<u32>,
    #[serde(rename = "descriptionEndLine", skip_serializing_if = "Option::is_none")]
    description_end_line: Option<u32>,
    #[serde(
        rename = "lastDescriptionLine",
        skip_serializing_if = "Option::is_none"
    )]
    last_description_line: Option<u32>,
    #[serde(
        rename = "hasPreterminalDescription",
        skip_serializing_if = "Option::is_none"
    )]
    has_preterminal_description: Option<u8>,
    #[serde(
        rename = "hasPreterminalTagDescription",
        skip_serializing_if = "Option::is_none"
    )]
    has_preterminal_tag_description: Option<u8>,

    description: &'a str,
    #[serde(rename = "descriptionLines")]
    description_lines: Vec<SerDescriptionLine<'a>>,
    tags: Vec<SerTag<'a>>,
    #[serde(rename = "inlineTags")]
    inline_tags: Vec<SerInlineTag<'a>>,
}

impl<'a> SerBlock<'a> {
    fn new(block: &'a JsdocBlock<'a>, opts: &SerializeOptions) -> Self {
        let pos = opts.include_positions;
        let compat = opts.compat_mode;

        Self {
            node_type: "JsdocBlock",
            start: if pos { Some(block.span.start) } else { None },
            end: if pos { Some(block.span.end) } else { None },
            range: if pos {
                Some([block.span.start, block.span.end])
            } else {
                None
            },
            delimiter: if compat { Some(block.delimiter) } else { None },
            post_delimiter: if compat {
                Some(block.post_delimiter)
            } else {
                None
            },
            initial: if compat { Some(block.initial) } else { None },
            terminal: if compat { Some(block.terminal) } else { None },
            line_end: if compat { Some(block.line_end) } else { None },
            delimiter_line_break: if compat {
                Some(block.delimiter_line_break)
            } else {
                None
            },
            preterminal_line_break: if compat {
                Some(block.preterminal_line_break)
            } else {
                None
            },
            end_line: if compat { Some(block.end_line) } else { None },
            description_start_line: if compat {
                block.description_start_line
            } else {
                None
            },
            description_end_line: if compat {
                block.description_end_line
            } else {
                None
            },
            last_description_line: if compat {
                block.last_description_line
            } else {
                None
            },
            has_preterminal_description: if compat {
                Some(block.has_preterminal_description)
            } else {
                None
            },
            has_preterminal_tag_description: if compat {
                block.has_preterminal_tag_description
            } else {
                None
            },
            description: block.description.unwrap_or_default(),
            description_lines: block
                .description_lines
                .iter()
                .map(|l| SerDescriptionLine::new(l, opts))
                .collect(),
            tags: block.tags.iter().map(|t| SerTag::new(t, opts)).collect(),
            inline_tags: block
                .inline_tags
                .iter()
                .map(|t| SerInlineTag::new(t, opts))
                .collect(),
        }
    }
}

// Keep backward-compatible From impl for existing callers.
impl<'a> From<&'a JsdocBlock<'a>> for SerBlock<'a> {
    fn from(block: &'a JsdocBlock<'a>) -> Self {
        Self::new(block, &SerializeOptions::default())
    }
}

#[derive(Serialize)]
struct SerDescriptionLine<'a> {
    #[serde(rename = "type")]
    node_type: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    start: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    end: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    range: Option<[u32; 2]>,
    delimiter: &'a str,
    #[serde(rename = "postDelimiter")]
    post_delimiter: &'a str,
    initial: &'a str,
    description: &'a str,
}

impl<'a> SerDescriptionLine<'a> {
    fn new(line: &'a JsdocDescriptionLine<'a>, opts: &SerializeOptions) -> Self {
        let pos = opts.include_positions;
        Self {
            node_type: "JsdocDescriptionLine",
            start: if pos { Some(line.span.start) } else { None },
            end: if pos { Some(line.span.end) } else { None },
            range: if pos {
                Some([line.span.start, line.span.end])
            } else {
                None
            },
            delimiter: line.delimiter,
            post_delimiter: line.post_delimiter,
            initial: line.initial,
            description: line.description,
        }
    }
}

impl<'a> From<&'a JsdocDescriptionLine<'a>> for SerDescriptionLine<'a> {
    fn from(line: &'a JsdocDescriptionLine<'a>) -> Self {
        Self::new(line, &SerializeOptions::default())
    }
}

#[derive(Serialize)]
struct SerTag<'a> {
    #[serde(rename = "type")]
    node_type: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    start: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    end: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    range: Option<[u32; 2]>,
    tag: &'a str,
    #[serde(rename = "rawType")]
    raw_type: Option<&'a str>,
    #[serde(rename = "parsedType", skip_serializing_if = "Option::is_none")]
    parsed_type: Option<serde_json::Value>,
    name: Option<&'a str>,

    // ox-jsdoc specific fields (excluded in compat_mode)
    #[serde(skip_serializing_if = "Option::is_none")]
    optional: Option<bool>,
    #[serde(rename = "defaultValue", skip_serializing_if = "Option::is_none")]
    default_value: Option<&'a str>,

    description: &'a str,

    #[serde(rename = "rawBody", skip_serializing_if = "Option::is_none")]
    raw_body: Option<&'a str>,

    // compat_mode fields
    #[serde(skip_serializing_if = "Option::is_none")]
    delimiter: Option<&'a str>,
    #[serde(rename = "postDelimiter", skip_serializing_if = "Option::is_none")]
    post_delimiter: Option<&'a str>,
    #[serde(rename = "postTag", skip_serializing_if = "Option::is_none")]
    post_tag: Option<&'a str>,
    #[serde(rename = "postType", skip_serializing_if = "Option::is_none")]
    post_type: Option<&'a str>,
    #[serde(rename = "postName", skip_serializing_if = "Option::is_none")]
    post_name: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    initial: Option<&'a str>,
    #[serde(rename = "lineEnd", skip_serializing_if = "Option::is_none")]
    line_end: Option<&'a str>,

    #[serde(rename = "typeLines")]
    type_lines: Vec<SerTypeLine<'a>>,
    #[serde(rename = "descriptionLines")]
    description_lines: Vec<SerDescriptionLine<'a>>,
    #[serde(rename = "inlineTags")]
    inline_tags: Vec<SerInlineTag<'a>>,

    #[serde(skip_serializing_if = "Option::is_none")]
    body: Option<SerTagBody<'a>>,
}

impl<'a> SerTag<'a> {
    fn new(tag: &'a JsdocTag<'a>, opts: &SerializeOptions) -> Self {
        let pos = opts.include_positions;
        let compat = opts.compat_mode;
        let null_to_empty = opts.empty_string_for_null;

        let raw_type_val = tag.raw_type.map(|rt| rt.raw);
        let name_val = tag.name.map(|n| n.raw);

        Self {
            node_type: "JsdocTag",
            start: if pos { Some(tag.span.start) } else { None },
            end: if pos { Some(tag.span.end) } else { None },
            range: if pos {
                Some([tag.span.start, tag.span.end])
            } else {
                None
            },
            tag: tag.tag.value,
            raw_type: if null_to_empty {
                Some(raw_type_val.unwrap_or(""))
            } else {
                raw_type_val
            },
            parsed_type: tag.parsed_type.as_ref().and_then(|pt| {
                match pt.as_ref() {
                    JsdocType::Parsed(node) => Some(serialize_type_node(node)),
                    JsdocType::Raw(_) => None,
                }
            }),
            name: if null_to_empty {
                Some(name_val.unwrap_or(""))
            } else {
                name_val
            },
            optional: if compat { None } else { Some(tag.optional) },
            default_value: if compat { None } else { tag.default_value },
            description: tag.description.unwrap_or_default(),
            raw_body: if compat { None } else { tag.raw_body },
            delimiter: if compat { Some(tag.delimiter) } else { None },
            post_delimiter: if compat {
                Some(tag.post_delimiter)
            } else {
                None
            },
            post_tag: if compat { Some(tag.post_tag) } else { None },
            post_type: if compat { Some(tag.post_type) } else { None },
            post_name: if compat { Some(tag.post_name) } else { None },
            initial: if compat { Some(tag.initial) } else { None },
            line_end: if compat { Some(tag.line_end) } else { None },
            type_lines: tag
                .type_lines
                .iter()
                .map(|l| SerTypeLine::new(l, opts))
                .collect(),
            description_lines: tag
                .description_lines
                .iter()
                .map(|l| SerDescriptionLine::new(l, opts))
                .collect(),
            inline_tags: tag
                .inline_tags
                .iter()
                .map(|t| SerInlineTag::new(t, opts))
                .collect(),
            body: if compat {
                None
            } else {
                tag.body.as_ref().map(|b| SerTagBody::from(b.as_ref()))
            },
        }
    }
}

impl<'a> From<&'a JsdocTag<'a>> for SerTag<'a> {
    fn from(tag: &'a JsdocTag<'a>) -> Self {
        Self::new(tag, &SerializeOptions::default())
    }
}

#[derive(Serialize)]
struct SerTypeLine<'a> {
    #[serde(rename = "type")]
    node_type: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    start: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    end: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    range: Option<[u32; 2]>,
    delimiter: &'a str,
    #[serde(rename = "postDelimiter")]
    post_delimiter: &'a str,
    initial: &'a str,
    #[serde(rename = "rawType")]
    raw_type: &'a str,
}

impl<'a> SerTypeLine<'a> {
    fn new(line: &'a JsdocTypeLine<'a>, opts: &SerializeOptions) -> Self {
        let pos = opts.include_positions;
        Self {
            node_type: "JsdocTypeLine",
            start: if pos { Some(line.span.start) } else { None },
            end: if pos { Some(line.span.end) } else { None },
            range: if pos {
                Some([line.span.start, line.span.end])
            } else {
                None
            },
            delimiter: line.delimiter,
            post_delimiter: line.post_delimiter,
            initial: line.initial,
            raw_type: line.raw_type,
        }
    }
}

impl<'a> From<&'a JsdocTypeLine<'a>> for SerTypeLine<'a> {
    fn from(line: &'a JsdocTypeLine<'a>) -> Self {
        Self::new(line, &SerializeOptions::default())
    }
}

#[derive(Serialize)]
struct SerInlineTag<'a> {
    #[serde(rename = "type")]
    node_type: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    start: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    end: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    range: Option<[u32; 2]>,
    tag: &'a str,
    #[serde(rename = "namepathOrURL")]
    namepath_or_url: Option<&'a str>,
    text: Option<&'a str>,
    format: &'static str,
    #[serde(rename = "rawBody", skip_serializing_if = "Option::is_none")]
    raw_body: Option<&'a str>,
}

impl<'a> SerInlineTag<'a> {
    fn new(tag: &'a JsdocInlineTag<'a>, opts: &SerializeOptions) -> Self {
        let pos = opts.include_positions;
        let compat = opts.compat_mode;
        let null_to_empty = opts.empty_string_for_null;

        Self {
            node_type: "JsdocInlineTag",
            start: if pos { Some(tag.span.start) } else { None },
            end: if pos { Some(tag.span.end) } else { None },
            range: if pos {
                Some([tag.span.start, tag.span.end])
            } else {
                None
            },
            tag: tag.tag.value,
            namepath_or_url: if null_to_empty {
                Some(tag.namepath_or_url.unwrap_or(""))
            } else {
                tag.namepath_or_url
            },
            text: if null_to_empty {
                Some(tag.text.unwrap_or(""))
            } else {
                tag.text
            },
            format: match tag.format {
                JsdocInlineTagFormat::Plain => "plain",
                JsdocInlineTagFormat::Pipe => "pipe",
                JsdocInlineTagFormat::Space => "space",
                JsdocInlineTagFormat::Prefix => "prefix",
                // In compat_mode, map Unknown to "plain" (Phase 5)
                JsdocInlineTagFormat::Unknown => {
                    if compat {
                        "plain"
                    } else {
                        "unknown"
                    }
                }
            },
            raw_body: if compat { None } else { tag.raw_body },
        }
    }
}

impl<'a> From<&'a JsdocInlineTag<'a>> for SerInlineTag<'a> {
    fn from(tag: &'a JsdocInlineTag<'a>) -> Self {
        Self::new(tag, &SerializeOptions::default())
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

// ---------------------------------------------------------------------------
// TypeNode serialization — jsdoc-type-pratt-parser compatible JSON
// ---------------------------------------------------------------------------

/// Build a JSON object, omitting any fields with null values.
fn json_obj(fields: Vec<(&str, serde_json::Value)>) -> serde_json::Value {
    let mut map = serde_json::Map::new();
    for (key, value) in fields {
        if !value.is_null() {
            map.insert(key.into(), value);
        }
    }
    serde_json::Value::Object(map)
}

/// Build a meta object, omitting null fields. Returns None if empty.
fn meta_obj(fields: Vec<(&str, serde_json::Value)>) -> Option<serde_json::Value> {
    let mut map = serde_json::Map::new();
    for (key, value) in fields {
        if !value.is_null() {
            map.insert(key.into(), value);
        }
    }
    if map.is_empty() { None } else { Some(serde_json::Value::Object(map)) }
}

fn serialize_type_node(node: &TypeNode<'_>) -> serde_json::Value {
    use serde_json::json;

    match node {
        TypeNode::Name(n) => json!({
            "type": "JsdocTypeName",
            "value": n.value,
        }),
        TypeNode::Number(n) => json!({
            "type": "JsdocTypeNumber",
            "value": n.value.parse::<f64>().unwrap_or(0.0),
        }),
        TypeNode::StringValue(n) => json!({
            "type": "JsdocTypeStringValue",
            "value": unquote(n.value),
            "meta": { "quote": quote_str(n.quote) },
        }),
        TypeNode::Null(_) => json!({ "type": "JsdocTypeNull" }),
        TypeNode::Undefined(_) => json!({ "type": "JsdocTypeUndefined" }),
        TypeNode::Any(_) => json!({ "type": "JsdocTypeAny" }),
        TypeNode::Unknown(_) => json!({ "type": "JsdocTypeUnknown" }),

        TypeNode::Union(n) => json!({
            "type": "JsdocTypeUnion",
            "elements": n.elements.iter().map(|e| serialize_type_node(e)).collect::<Vec<_>>(),
        }),
        TypeNode::Intersection(n) => json!({
            "type": "JsdocTypeIntersection",
            "elements": n.elements.iter().map(|e| serialize_type_node(e)).collect::<Vec<_>>(),
        }),
        TypeNode::Generic(n) => {
            // For square bracket syntax (T[]), jsdoc-type-pratt-parser outputs
            // left: { type: "JsdocTypeName", value: "Array" }, elements: [T]
            if n.brackets == GenericBrackets::Square && n.elements.is_empty() {
                json!({
                    "type": "JsdocTypeGeneric",
                    "left": json!({"type": "JsdocTypeName", "value": "Array"}),
                    "elements": [serialize_type_node(&n.left)],
                    "meta": { "brackets": "square", "dot": false },
                })
            } else {
                json!({
                    "type": "JsdocTypeGeneric",
                    "left": serialize_type_node(&n.left),
                    "elements": n.elements.iter().map(|e| serialize_type_node(e)).collect::<Vec<_>>(),
                    "meta": {
                        "brackets": match n.brackets {
                            GenericBrackets::Angle => "angle",
                            GenericBrackets::Square => "square",
                        },
                        "dot": n.dot,
                    },
                })
            }
        }
        TypeNode::Function(n) => {
            let mut obj = json!({
                "type": "JsdocTypeFunction",
                "parameters": n.parameters.iter().map(|e| serialize_type_node(e)).collect::<Vec<_>>(),
                "constructor": n.constructor,
                "arrow": n.arrow,
                "parenthesis": n.parenthesis,
            });
            if let Some(ref ret) = n.return_type {
                obj["returnType"] = serialize_type_node(ret);
            }
            obj
        }
        TypeNode::Object(n) => json!({
            "type": "JsdocTypeObject",
            "elements": n.elements.iter().map(|e| serialize_type_node(e)).collect::<Vec<_>>(),
            "meta": {
                "separator": match n.separator {
                    Some(ObjectSeparator::Semicolon) => "semicolon",
                    Some(ObjectSeparator::Linebreak) => "linebreak",
                    Some(ObjectSeparator::CommaAndLinebreak) => "comma-and-linebreak",
                    Some(ObjectSeparator::SemicolonAndLinebreak) => "semicolon-and-linebreak",
                    _ => "comma", // Default to "comma" (matches jsdoc-type-pratt-parser)
                },
            },
        }),
        TypeNode::Tuple(n) => json!({
            "type": "JsdocTypeTuple",
            "elements": n.elements.iter().map(|e| serialize_type_node(e)).collect::<Vec<_>>(),
        }),
        TypeNode::Parenthesis(n) => json!({
            "type": "JsdocTypeParenthesis",
            "element": serialize_type_node(&n.element),
        }),

        TypeNode::NamePath(n) => json!({
            "type": "JsdocTypeNamePath",
            "left": serialize_type_node(&n.left),
            "right": serialize_type_node(&n.right),
            "pathType": match n.path_type {
                NamePathType::Property => "property",
                NamePathType::Instance => "instance",
                NamePathType::Inner => "inner",
                NamePathType::PropertyBrackets => "property-brackets",
            },
        }),
        TypeNode::SpecialNamePath(n) => {
            let mut obj = json!({
                "type": "JsdocTypeSpecialNamePath",
                "value": n.value,
                "specialType": match n.special_type {
                    SpecialPathType::Module => "module",
                    SpecialPathType::Event => "event",
                    SpecialPathType::External => "external",
                },
            });
            if let Some(meta) = meta_obj(vec![
                ("quote", n.quote.map_or(serde_json::Value::Null, |q| json!(quote_str(q)))),
            ]) {
                obj["meta"] = meta;
            }
            obj
        }

        TypeNode::Nullable(n) => json!({
            "type": "JsdocTypeNullable",
            "element": serialize_type_node(&n.element),
            "meta": { "position": position_str(n.position) },
        }),
        TypeNode::NotNullable(n) => json!({
            "type": "JsdocTypeNotNullable",
            "element": serialize_type_node(&n.element),
            "meta": { "position": position_str(n.position) },
        }),
        TypeNode::Optional(n) => json!({
            "type": "JsdocTypeOptional",
            "element": serialize_type_node(&n.element),
            "meta": { "position": position_str(n.position) },
        }),
        TypeNode::Variadic(n) => {
            let mut obj = json!({
                "type": "JsdocTypeVariadic",
            });
            if let Some(ref element) = n.element {
                obj["element"] = serialize_type_node(element);
            }
            let pos_val = n.position.map_or(serde_json::Value::Null, |p| json!(match p {
                VariadicPosition::Prefix => "prefix",
                VariadicPosition::Suffix => "suffix",
            }));
            if let Some(meta) = meta_obj(vec![
                ("position", pos_val),
                ("squareBrackets", json!(n.square_brackets)),
            ]) {
                obj["meta"] = meta;
            }
            obj
        }

        TypeNode::Conditional(n) => json!({
            "type": "JsdocTypeConditional",
            "checksType": serialize_type_node(&n.checks_type),
            "extendsType": serialize_type_node(&n.extends_type),
            "trueType": serialize_type_node(&n.true_type),
            "falseType": serialize_type_node(&n.false_type),
        }),
        TypeNode::Infer(n) => json!({
            "type": "JsdocTypeInfer",
            "element": serialize_type_node(&n.element),
        }),
        TypeNode::KeyOf(n) => json!({
            "type": "JsdocTypeKeyof",
            "element": serialize_type_node(&n.element),
        }),
        TypeNode::TypeOf(n) => json!({
            "type": "JsdocTypeTypeof",
            "element": serialize_type_node(&n.element),
        }),
        TypeNode::Import(n) => json!({
            "type": "JsdocTypeImport",
            "element": serialize_type_node(&n.element),
        }),
        TypeNode::Predicate(n) => json!({
            "type": "JsdocTypePredicate",
            "left": serialize_type_node(&n.left),
            "right": serialize_type_node(&n.right),
        }),
        TypeNode::Asserts(n) => json!({
            "type": "JsdocTypeAsserts",
            "left": serialize_type_node(&n.left),
            "right": serialize_type_node(&n.right),
        }),
        TypeNode::AssertsPlain(n) => json!({
            "type": "JsdocTypeAssertsPlain",
            "element": serialize_type_node(&n.element),
        }),
        TypeNode::ReadonlyArray(n) => json!({
            "type": "JsdocTypeReadonlyArray",
            "element": serialize_type_node(&n.element),
        }),
        TypeNode::TemplateLiteral(n) => json!({
            "type": "JsdocTypeTemplateLiteral",
            "literals": n.literals.iter().collect::<Vec<_>>(),
            "interpolations": n.interpolations.iter().map(|e| serialize_type_node(e)).collect::<Vec<_>>(),
        }),
        TypeNode::UniqueSymbol(_) => json!({ "type": "JsdocTypeUniqueSymbol" }),

        TypeNode::Symbol(n) => {
            let mut obj = json!({
                "type": "JsdocTypeSymbol",
                "value": n.value,
            });
            if let Some(ref element) = n.element {
                obj["element"] = serialize_type_node(element);
            }
            obj
        }

        TypeNode::ObjectField(n) => {
            let key_str = match n.key.as_ref() {
                TypeNode::Name(name) => name.value,
                TypeNode::StringValue(sv) => unquote(sv.value),
                TypeNode::Number(num) => num.value,
                _ => "",
            };
            let mut obj = json!({
                "type": "JsdocTypeObjectField",
                "key": key_str,
                "optional": n.optional,
                "readonly": n.readonly,
            });
            if let Some(ref right) = n.right {
                obj["right"] = serialize_type_node(right);
            }
            if let Some(meta) = meta_obj(vec![
                ("quote", n.quote.map_or(serde_json::Value::Null, |q| json!(quote_str(q)))),
            ]) {
                obj["meta"] = meta;
            }
            obj
        }
        TypeNode::JsdocObjectField(n) => json!({
            "type": "JsdocTypeJsdocObjectField",
            "left": serialize_type_node(&n.left),
            "right": serialize_type_node(&n.right),
        }),
        TypeNode::KeyValue(n) => {
            let mut obj = json!({
                "type": "JsdocTypeKeyValue",
                "key": n.key,
                "optional": n.optional,
                "variadic": n.variadic,
            });
            if let Some(ref right) = n.right {
                obj["right"] = serialize_type_node(right);
            }
            obj
        }
        TypeNode::Property(n) => {
            let mut obj = json!({
                "type": "JsdocTypeProperty",
                "value": n.value,
            });
            if let Some(meta) = meta_obj(vec![
                ("quote", n.quote.map_or(serde_json::Value::Null, |q| json!(quote_str(q)))),
            ]) {
                obj["meta"] = meta;
            }
            obj
        }
        TypeNode::IndexSignature(n) => json!({
            "type": "JsdocTypeIndexSignature",
            "key": n.key,
            "right": serialize_type_node(&n.right),
        }),
        TypeNode::MappedType(n) => json!({
            "type": "JsdocTypeMappedType",
            "key": n.key,
            "right": serialize_type_node(&n.right),
        }),
        TypeNode::TypeParameter(n) => {
            let mut obj = json!({
                "type": "JsdocTypeTypeParameter",
                "name": serialize_type_node(&n.name),
            });
            if let Some(ref constraint) = n.constraint {
                obj["constraint"] = serialize_type_node(constraint);
            }
            if let Some(ref default) = n.default_value {
                obj["defaultValue"] = serialize_type_node(default);
            }
            obj
        }
        TypeNode::CallSignature(n) => json!({
            "type": "JsdocTypeCallSignature",
            "parameters": n.parameters.iter().map(|e| serialize_type_node(e)).collect::<Vec<_>>(),
            "returnType": serialize_type_node(&n.return_type),
            "typeParameters": n.type_parameters.iter().map(|e| serialize_type_node(e)).collect::<Vec<_>>(),
        }),
        TypeNode::ConstructorSignature(n) => json!({
            "type": "JsdocTypeConstructorSignature",
            "parameters": n.parameters.iter().map(|e| serialize_type_node(e)).collect::<Vec<_>>(),
            "returnType": serialize_type_node(&n.return_type),
            "typeParameters": n.type_parameters.iter().map(|e| serialize_type_node(e)).collect::<Vec<_>>(),
        }),
        TypeNode::MethodSignature(n) => {
            let mut obj = json!({
                "type": "JsdocTypeMethodSignature",
                "name": n.name,
                "parameters": n.parameters.iter().map(|e| serialize_type_node(e)).collect::<Vec<_>>(),
                "returnType": serialize_type_node(&n.return_type),
                "typeParameters": n.type_parameters.iter().map(|e| serialize_type_node(e)).collect::<Vec<_>>(),
            });
            if let Some(meta) = meta_obj(vec![
                ("quote", n.quote.map_or(serde_json::Value::Null, |q| json!(quote_str(q)))),
            ]) {
                obj["meta"] = meta;
            }
            obj
        }
        TypeNode::IndexedAccessIndex(n) => json!({
            "type": "JsdocTypeIndexedAccessIndex",
            "right": serialize_type_node(&n.right),
        }),
        TypeNode::ParameterList(n) => json!({
            "type": "JsdocTypeParameterList",
            "elements": n.elements.iter().map(|e| serialize_type_node(e)).collect::<Vec<_>>(),
        }),
        TypeNode::ReadonlyProperty(n) => json!({
            "type": "JsdocTypeReadonlyProperty",
            "element": serialize_type_node(&n.element),
        }),
    }
}

fn quote_str(q: QuoteStyle) -> &'static str {
    match q {
        QuoteStyle::Single => "single",
        QuoteStyle::Double => "double",
    }
}

fn position_str(p: ModifierPosition) -> &'static str {
    match p {
        ModifierPosition::Prefix => "prefix",
        ModifierPosition::Suffix => "suffix",
    }
}

/// Remove surrounding quotes from a string literal value.
fn unquote(s: &str) -> &str {
    if (s.starts_with('"') && s.ends_with('"')) || (s.starts_with('\'') && s.ends_with('\'')) {
        &s[1..s.len() - 1]
    } else {
        s
    }
}

#[cfg(test)]
mod tests {
    use oxc_allocator::Allocator;

    use crate::analyzer::analyze_comment;
    use crate::parse_comment;
    use crate::parser::ParseOptions;
    use crate::validator::{ValidationOptions, validate_comment};

    use super::{
        SerializeOptions, SpacingMode, serialize_comment_json, serialize_comment_json_with_options,
    };

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

    #[test]
    fn compat_mode_outputs_jsdoccomment_fields() {
        let allocator = Allocator::default();
        let parsed = parse_comment(
            &allocator,
            "/**\n * Description\n * @param {string} id - The user ID\n */",
            0,
            ParseOptions::default(),
        );
        let comment = parsed.comment.expect("expected comment");

        let opts = SerializeOptions {
            compat_mode: true,
            empty_string_for_null: true,
            include_positions: true,
            spacing: SpacingMode::Compact,
        };
        let json = serialize_comment_json_with_options(&comment, None, None, &opts);

        // JsdocBlock compat fields present
        assert!(json.contains("\"delimiter\":\"/**\""));
        assert!(json.contains("\"terminal\":\"*/\""));
        assert!(json.contains("\"delimiterLineBreak\":"));
        assert!(json.contains("\"preterminalLineBreak\":"));
        assert!(json.contains("\"endLine\":"));
        assert!(json.contains("\"hasPreterminalDescription\":"));

        // JsdocTag compat fields present
        assert!(json.contains("\"postTag\":"));
        assert!(json.contains("\"postType\":"));
        assert!(json.contains("\"postName\":"));
        assert!(json.contains("\"lineEnd\":"));

        // ox-jsdoc specific fields excluded
        assert!(!json.contains("\"optional\":"));
        assert!(!json.contains("\"rawBody\":"));
        assert!(!json.contains("\"body\":"));
    }

    #[test]
    fn compat_mode_empty_string_for_null() {
        let allocator = Allocator::default();
        let parsed = parse_comment(
            &allocator,
            "/** @returns void */",
            0,
            ParseOptions::default(),
        );
        let comment = parsed.comment.expect("expected comment");

        let opts = SerializeOptions {
            compat_mode: true,
            empty_string_for_null: true,
            ..SerializeOptions::default()
        };
        let json = serialize_comment_json_with_options(&comment, None, None, &opts);

        // rawType should be present (not null-omitted)
        assert!(json.contains("\"rawType\":"));
    }

    #[test]
    fn serializes_parsed_type_simple_name() {
        use crate::type_parser::ast::ParseMode;

        let allocator = Allocator::default();
        let parsed = parse_comment(
            &allocator,
            "/** @param {string} id */",
            0,
            ParseOptions {
                parse_types: true,
                type_parse_mode: ParseMode::Jsdoc,
                ..ParseOptions::default()
            },
        );
        let comment = parsed.comment.expect("expected comment");
        let json = serialize_comment_json(&comment, None, None);

        assert!(json.contains("\"parsedType\":{\"type\":\"JsdocTypeName\",\"value\":\"string\"}"));
    }

    #[test]
    fn serializes_parsed_type_union() {
        use crate::type_parser::ast::ParseMode;

        let allocator = Allocator::default();
        let parsed = parse_comment(
            &allocator,
            "/** @param {string | number} id */",
            0,
            ParseOptions {
                parse_types: true,
                type_parse_mode: ParseMode::Typescript,
                ..ParseOptions::default()
            },
        );
        let comment = parsed.comment.expect("expected comment");
        let json = serialize_comment_json(&comment, None, None);

        assert!(json.contains("\"JsdocTypeUnion\""), "JSON: {json}");
        assert!(json.contains("\"parsedType\""), "parsedType missing from JSON: {json}");
    }

    #[test]
    fn serializes_no_parsed_type_when_disabled() {
        let allocator = Allocator::default();
        let parsed = parse_comment(
            &allocator,
            "/** @param {string} id */",
            0,
            ParseOptions::default(), // parse_types: false
        );
        let comment = parsed.comment.expect("expected comment");
        let json = serialize_comment_json(&comment, None, None);

        // parsedType should not appear in JSON
        assert!(!json.contains("\"parsedType\""));
    }
}
