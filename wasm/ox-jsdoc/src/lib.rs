// @author kazuya kawaguchi (a.k.a. kazupon)
// @license MIT
//

//! WASM binding for ox-jsdoc.

use wasm_bindgen::prelude::*;

use oxc_allocator::Allocator;

use ox_jsdoc::type_parser::stringify::stringify_type;
use ox_jsdoc::{ParseMode, ParseOptions, parse_comment, parse_type, serialize_comment_json};

/// Parse a JSDoc block comment.
///
/// Returns an object with `astJson` (string) and `diagnostics` (array).
#[wasm_bindgen]
pub fn parse(
    source_text: &str,
    fence_aware: Option<bool>,
    parse_types: Option<bool>,
    type_parse_mode: Option<String>,
) -> JsValue {
    let allocator = Allocator::default();
    let mode = match type_parse_mode.as_deref() {
        Some("typescript") => ParseMode::Typescript,
        Some("closure") => ParseMode::Closure,
        _ => ParseMode::Jsdoc,
    };
    let options = ParseOptions {
        fence_aware: fence_aware.unwrap_or(true),
        parse_types: parse_types.unwrap_or(false),
        type_parse_mode: mode,
        ..ParseOptions::default()
    };
    let output = parse_comment(&allocator, source_text, 0, options);

    let ast_json = match output.comment {
        Some(ref comment) => serialize_comment_json(comment, None, None),
        None => "null".to_string(),
    };

    let result = js_sys::Object::new();
    js_sys::Reflect::set(&result, &"astJson".into(), &ast_json.into()).unwrap();

    let diag_array = js_sys::Array::new();
    for d in &output.diagnostics {
        let obj = js_sys::Object::new();
        js_sys::Reflect::set(&obj, &"message".into(), &d.to_string().into()).unwrap();
        diag_array.push(&obj);
    }
    js_sys::Reflect::set(&result, &"diagnostics".into(), &diag_array.into()).unwrap();

    result.into()
}

/// Parse a standalone type expression.
/// Returns the stringified type or null if parsing fails.
#[wasm_bindgen]
pub fn parse_type_expression(type_text: &str, mode: Option<String>) -> Option<String> {
    let allocator = Allocator::default();
    let parse_mode = match mode.as_deref() {
        Some("typescript") => ParseMode::Typescript,
        Some("closure") => ParseMode::Closure,
        _ => ParseMode::Jsdoc,
    };
    let output = parse_type(&allocator, type_text, 0, parse_mode);
    output.node.map(|node| stringify_type(&node))
}

/// Parse a type expression and return whether it succeeded.
/// No stringify overhead — used for benchmarks.
#[wasm_bindgen]
pub fn parse_type_check(type_text: &str, mode: Option<String>) -> bool {
    let allocator = Allocator::default();
    let parse_mode = match mode.as_deref() {
        Some("typescript") => ParseMode::Typescript,
        Some("closure") => ParseMode::Closure,
        _ => ParseMode::Jsdoc,
    };
    let output = parse_type(&allocator, type_text, 0, parse_mode);
    output.node.is_some()
}
