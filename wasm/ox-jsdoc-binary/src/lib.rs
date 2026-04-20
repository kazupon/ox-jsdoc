// @author kazuya kawaguchi (a.k.a. kazupon)
// @license MIT
//

//! WASM binding for the binary AST flavor of ox-jsdoc.
//!
//! Returns a [`ParseResult`] handle whose `buffer_ptr`/`buffer_len` getters
//! let JS construct a `Uint8Array` view directly into `wasm.memory.buffer`
//! — no copy, matching `design/007-binary-ast/rust-impl.md` "Sharing with
//! NAPI/WASM" guidance.
//!
//! Lifecycle: the JS-side wrapper calls `ParseResult::free()` (auto-generated
//! by wasm-bindgen) once it is done reading, releasing the bytes.

use wasm_bindgen::prelude::*;

use oxc_allocator::Allocator;

use ox_jsdoc_binary::parser::{parse, type_data::ParseMode, ParseOptions};

/// Owned binary-AST result. JS reads `buffer_ptr` + `buffer_len` and views
/// the bytes via `new Uint8Array(wasm.memory.buffer, ptr, len)`. The bytes
/// stay alive until JS calls `result.free()`.
#[wasm_bindgen]
pub struct ParseResult {
    /// Heap-owned byte slice that backs the JS view.
    bytes: Box<[u8]>,
    /// Pre-rendered diagnostic messages.
    diagnostics: Vec<String>,
}

#[wasm_bindgen]
impl ParseResult {
    /// Pointer to the first byte of the binary AST inside `wasm.memory.buffer`.
    #[wasm_bindgen(js_name = bufferPtr)]
    #[must_use]
    pub fn buffer_ptr(&self) -> *const u8 {
        self.bytes.as_ptr()
    }

    /// Length of the binary AST in bytes.
    #[wasm_bindgen(js_name = bufferLen)]
    #[must_use]
    pub fn buffer_len(&self) -> usize {
        self.bytes.len()
    }

    /// Diagnostics produced during parsing (one entry per recoverable error).
    #[wasm_bindgen]
    #[must_use]
    pub fn diagnostics(&self) -> Vec<JsValue> {
        self.diagnostics
            .iter()
            .map(|message| {
                let obj = js_sys::Object::new();
                js_sys::Reflect::set(&obj, &JsValue::from_str("message"), &JsValue::from_str(message))
                    .expect("set message");
                obj.into()
            })
            .collect()
    }
}

/// Parse a complete JSDoc block comment into a binary-AST byte buffer.
#[wasm_bindgen]
pub fn parse_jsdoc(
    source_text: &str,
    fence_aware: Option<bool>,
    parse_types: Option<bool>,
    type_parse_mode: Option<String>,
    compat_mode: Option<bool>,
    base_offset: Option<u32>,
) -> ParseResult {
    let arena = Allocator::default();
    let mode = match type_parse_mode.as_deref() {
        Some("typescript") => ParseMode::Typescript,
        Some("closure") => ParseMode::Closure,
        _ => ParseMode::Jsdoc,
    };
    let options = ParseOptions {
        compat_mode: compat_mode.unwrap_or(false),
        base_offset: base_offset.unwrap_or(0),
        fence_aware: fence_aware.unwrap_or(true),
        parse_types: parse_types.unwrap_or(false),
        type_parse_mode: mode,
    };
    let result = parse(&arena, source_text, options);

    let diagnostics = result
        .diagnostics
        .iter()
        .map(|d| d.message.to_string())
        .collect();
    let bytes: Box<[u8]> = result.binary_bytes.to_vec().into_boxed_slice();

    ParseResult { bytes, diagnostics }
}
