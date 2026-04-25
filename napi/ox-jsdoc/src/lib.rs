// @author kazuya kawaguchi (a.k.a. kazupon)
// @license MIT
//

//! NAPI binding for ox-jsdoc.

use napi_derive::napi;
use oxc_allocator::Allocator;

use ox_jsdoc::type_parser::stringify::stringify_type;
use ox_jsdoc::{
    ParseMode, ParseOptions, SerializeOptions, SpacingMode, parse_comment, parse_type,
    serialize_comment_json_with_options,
};

#[napi(object)]
#[derive(Default)]
pub struct JsParseOptions {
    /// Suppress tag recognition inside fenced code blocks. Default: true.
    pub fence_aware: Option<bool>,
    /// Enable type expression parsing for `{...}` in tags. Default: false.
    pub parse_types: Option<bool>,
    /// Parse mode for type expressions: "jsdoc", "closure", or "typescript". Default: "jsdoc".
    pub type_parse_mode: Option<String>,
    /// Output jsdoccomment-compatible fields (delimiter, postDelimiter,
    /// initial, line indices, …) and exclude ox-jsdoc-specific fields
    /// (optional, defaultValue, rawBody, body). Default: false.
    pub compat_mode: Option<bool>,
    /// When true, optional string fields that would normally be `null`
    /// (rawType, name, namepathOrURL, text) are emitted as `""` instead.
    /// Mirrors jsdoccomment's serialization. Default: false.
    pub empty_string_for_null: Option<bool>,
    /// Include ESTree position fields (start, end, range). Default: true.
    pub include_positions: Option<bool>,
    /// Spacing mode for compat output: "compact" (default, drops empty
    /// description lines like jsdoccomment) or "preserve" (keeps every
    /// scanned line verbatim). Only effective when `compat_mode` is true.
    pub spacing: Option<String>,
}

#[napi(object)]
pub struct JsDiagnostic {
    pub message: String,
}

#[napi(object)]
pub struct JsParseResult {
    pub ast_json: String,
    pub diagnostics: Vec<JsDiagnostic>,
}

/// Parse a complete `/** ... */` JSDoc block comment.
#[napi]
pub fn parse(source_text: String, options: Option<JsParseOptions>) -> JsParseResult {
    let allocator = Allocator::default();
    let serialize_opts = convert_serialize_options(options.as_ref());
    let opts = convert_options(options);
    let output = parse_comment(&allocator, &source_text, 0, opts);

    let (ast_json, mut diagnostics) = match output.comment {
        Some(ref comment) => {
            let json = serialize_comment_json_with_options(comment, None, None, &serialize_opts);
            (json, Vec::new())
        }
        None => ("null".to_string(), Vec::new()),
    };

    for d in &output.diagnostics {
        diagnostics.push(JsDiagnostic {
            message: d.to_string(),
        });
    }

    JsParseResult {
        ast_json,
        diagnostics,
    }
}

/// Parse a standalone type expression (no comment parsing overhead).
/// Returns the stringified result, or null if parsing fails.
#[napi]
pub fn parse_type_expression(type_text: String, mode: Option<String>) -> Option<String> {
    let allocator = Allocator::default();
    let parse_mode = match mode.as_deref() {
        Some("typescript") => ParseMode::Typescript,
        Some("closure") => ParseMode::Closure,
        _ => ParseMode::Jsdoc,
    };
    let output = parse_type(&allocator, &type_text, 0, parse_mode);
    output.node.map(|node| stringify_type(&node))
}

/// Parse a standalone type expression and return whether it succeeded.
/// No stringify overhead — used for benchmarks.
#[napi]
pub fn parse_type_check(type_text: String, mode: Option<String>) -> bool {
    let allocator = Allocator::default();
    let parse_mode = match mode.as_deref() {
        Some("typescript") => ParseMode::Typescript,
        Some("closure") => ParseMode::Closure,
        _ => ParseMode::Jsdoc,
    };
    let output = parse_type(&allocator, &type_text, 0, parse_mode);
    output.node.is_some()
}

fn convert_options(options: Option<JsParseOptions>) -> ParseOptions {
    let options = options.unwrap_or_default();
    let type_parse_mode = match options.type_parse_mode.as_deref() {
        Some("typescript") => ParseMode::Typescript,
        Some("closure") => ParseMode::Closure,
        _ => ParseMode::Jsdoc,
    };
    ParseOptions {
        fence_aware: options.fence_aware.unwrap_or(true),
        parse_types: options.parse_types.unwrap_or(false),
        type_parse_mode,
        ..ParseOptions::default()
    }
}

fn convert_serialize_options(options: Option<&JsParseOptions>) -> SerializeOptions {
    let mut serialize_opts = SerializeOptions::default();
    let Some(options) = options else {
        return serialize_opts;
    };
    if let Some(value) = options.compat_mode {
        serialize_opts.compat_mode = value;
    }
    if let Some(value) = options.empty_string_for_null {
        serialize_opts.empty_string_for_null = value;
    }
    if let Some(value) = options.include_positions {
        serialize_opts.include_positions = value;
    }
    serialize_opts.spacing = match options.spacing.as_deref() {
        Some("preserve") => SpacingMode::Preserve,
        _ => SpacingMode::Compact,
    };
    serialize_opts
}
