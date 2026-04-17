// @author kazuya kawaguchi (a.k.a. kazupon)
// @license MIT
//

//! NAPI binding for ox-jsdoc.

use napi_derive::napi;
use oxc_allocator::Allocator;

use ox_jsdoc::{ParseMode, ParseOptions, parse_comment, parse_type, serialize_comment_json};
use ox_jsdoc::type_parser::stringify::stringify_type;

#[napi(object)]
#[derive(Default)]
pub struct JsParseOptions {
    /// Suppress tag recognition inside fenced code blocks. Default: true.
    pub fence_aware: Option<bool>,
    /// Enable type expression parsing for `{...}` in tags. Default: false.
    pub parse_types: Option<bool>,
    /// Parse mode for type expressions: "jsdoc", "closure", or "typescript". Default: "jsdoc".
    pub type_parse_mode: Option<String>,
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
    let opts = convert_options(options);
    let output = parse_comment(&allocator, &source_text, 0, opts);

    let (ast_json, mut diagnostics) = match output.comment {
        Some(ref comment) => {
            let json = serialize_comment_json(comment, None, None);
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
