// @author kazuya kawaguchi (a.k.a. kazupon)
// @license MIT
//

//! WASM binding for ox-jsdoc.

use wasm_bindgen::prelude::*;

use oxc_allocator::Allocator;

use ox_jsdoc::{ParseOptions, parse_comment, serialize_comment_json};

/// Parse a JSDoc block comment.
///
/// Returns an object with `astJson` (string) and `diagnostics` (array).
#[wasm_bindgen]
pub fn parse(source_text: &str, fence_aware: Option<bool>) -> JsValue {
    let allocator = Allocator::default();
    let options = ParseOptions {
        fence_aware: fence_aware.unwrap_or(true),
        inline_code_aware: false,
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
