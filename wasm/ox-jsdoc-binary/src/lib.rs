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

use ox_jsdoc_binary::parser::{
    BatchItem, ParseOptions, parse_batch_to_bytes, parse_to_bytes, type_data::ParseMode,
};

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
                js_sys::Reflect::set(
                    &obj,
                    &JsValue::from_str("message"),
                    &JsValue::from_str(message),
                )
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
    preserve_whitespace: Option<bool>,
    base_offset: Option<u32>,
) -> ParseResult {
    let mode = match type_parse_mode.as_deref() {
        Some("typescript") => ParseMode::Typescript,
        Some("closure") => ParseMode::Closure,
        _ => ParseMode::Jsdoc,
    };
    let options = ParseOptions {
        compat_mode: compat_mode.unwrap_or(false),
        preserve_whitespace: preserve_whitespace.unwrap_or(false),
        base_offset: base_offset.unwrap_or(0),
        fence_aware: fence_aware.unwrap_or(true),
        parse_types: parse_types.unwrap_or(false),
        type_parse_mode: mode,
    };
    let result = parse_to_bytes(source_text, options);

    let diagnostics = result
        .diagnostics
        .iter()
        .map(|d| d.message.to_string())
        .collect();
    // `Vec::into_boxed_slice` shrinks-to-fit but does not copy the buffer;
    // the underlying allocation is the one produced by `BinaryWriter::finish`.
    let bytes: Box<[u8]> = result.binary_bytes.into_boxed_slice();

    ParseResult { bytes, diagnostics }
}

/// Owned batch result. The shape mirrors [`ParseResult`] but carries
/// per-diagnostic `root_index` so JS callers can correlate each diagnostic
/// with the matching `BatchItem` index.
#[wasm_bindgen]
pub struct BatchParseResult {
    bytes: Box<[u8]>,
    diagnostic_messages: Vec<String>,
    diagnostic_root_indices: Vec<u32>,
}

#[wasm_bindgen]
impl BatchParseResult {
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

    /// Diagnostics produced during the batch (one entry per recoverable
    /// error, in input order). Each object carries `message` and
    /// `rootIndex`.
    #[wasm_bindgen]
    #[must_use]
    pub fn diagnostics(&self) -> Vec<JsValue> {
        self.diagnostic_messages
            .iter()
            .zip(self.diagnostic_root_indices.iter())
            .map(|(message, &root_index)| {
                let obj = js_sys::Object::new();
                js_sys::Reflect::set(
                    &obj,
                    &JsValue::from_str("message"),
                    &JsValue::from_str(message),
                )
                .expect("set message");
                js_sys::Reflect::set(
                    &obj,
                    &JsValue::from_str("rootIndex"),
                    &JsValue::from_f64(f64::from(root_index)),
                )
                .expect("set rootIndex");
                obj.into()
            })
            .collect()
    }
}

/// Parse N JSDoc block comments into a single shared binary AST buffer.
///
/// `source_texts` and `base_offsets` are parallel arrays of equal length;
/// each pair `(source_texts[i], base_offsets[i])` corresponds to one
/// `BatchItem`. The JS-side wrapper splits the user's `BatchItem[]` into
/// these two arrays before calling.
#[wasm_bindgen]
pub fn parse_jsdoc_batch(
    source_texts: Vec<String>,
    base_offsets: Vec<u32>,
    fence_aware: Option<bool>,
    parse_types: Option<bool>,
    type_parse_mode: Option<String>,
    compat_mode: Option<bool>,
    preserve_whitespace: Option<bool>,
) -> BatchParseResult {
    assert_eq!(
        source_texts.len(),
        base_offsets.len(),
        "source_texts and base_offsets must be the same length"
    );

    let mode = match type_parse_mode.as_deref() {
        Some("typescript") => ParseMode::Typescript,
        Some("closure") => ParseMode::Closure,
        _ => ParseMode::Jsdoc,
    };
    let options = ParseOptions {
        compat_mode: compat_mode.unwrap_or(false),
        preserve_whitespace: preserve_whitespace.unwrap_or(false),
        base_offset: 0,
        fence_aware: fence_aware.unwrap_or(true),
        parse_types: parse_types.unwrap_or(false),
        type_parse_mode: mode,
    };

    let items: Vec<BatchItem<'_>> = source_texts
        .iter()
        .zip(base_offsets.iter())
        .map(|(s, &o)| BatchItem {
            source_text: s.as_str(),
            base_offset: o,
        })
        .collect();

    let result = parse_batch_to_bytes(&items, options);

    let diagnostic_messages: Vec<String> = result
        .diagnostics
        .iter()
        .map(|d| d.message.to_string())
        .collect();
    let diagnostic_root_indices: Vec<u32> =
        result.diagnostics.iter().map(|d| d.root_index).collect();

    BatchParseResult {
        bytes: result.binary_bytes.into_boxed_slice(),
        diagnostic_messages,
        diagnostic_root_indices,
    }
}

/// Parse N JSDoc block comments where the JS-side has already concatenated
/// every `source_text` into a single UTF-8 byte buffer.
///
/// This avoids the per-item `Vec<String>` JS→Wasm marshalling that
/// dominates the [`parse_jsdoc_batch`] entry's cross-boundary cost (each
/// `String` element pays a separate JS string → wasm linear-memory copy +
/// `String` wrapper allocation). Three slice handles replace 226 ×
/// `(String, u32)` element conversions:
///
/// - `concat`: every `source_text` UTF-8 byte concatenated, no separators.
/// - `offsets`: length `N + 1`. `concat[offsets[i]..offsets[i+1]]` is the
///   bytes for input item `i`.
/// - `base_offsets`: length `N`. Per-item `base_offset`.
///
/// Mirrors the NAPI binding's `parse_jsdoc_batch_raw`. The JS wrapper
/// (`parseBatch` in `index.js`) builds these via `TextEncoder`. Callers
/// that need the original ergonomic API can keep using
/// [`parse_jsdoc_batch`].
#[wasm_bindgen]
pub fn parse_jsdoc_batch_raw(
    concat: &[u8],
    offsets: &[u32],
    base_offsets: &[u32],
    fence_aware: Option<bool>,
    parse_types: Option<bool>,
    type_parse_mode: Option<String>,
    compat_mode: Option<bool>,
    preserve_whitespace: Option<bool>,
) -> BatchParseResult {
    let mode = match type_parse_mode.as_deref() {
        Some("typescript") => ParseMode::Typescript,
        Some("closure") => ParseMode::Closure,
        _ => ParseMode::Jsdoc,
    };
    let options = ParseOptions {
        compat_mode: compat_mode.unwrap_or(false),
        preserve_whitespace: preserve_whitespace.unwrap_or(false),
        base_offset: 0,
        fence_aware: fence_aware.unwrap_or(true),
        parse_types: parse_types.unwrap_or(false),
        type_parse_mode: mode,
    };

    let n = base_offsets.len();
    debug_assert_eq!(offsets.len(), n + 1, "offsets must be length N + 1");

    let items: Vec<BatchItem<'_>> = (0..n)
        .map(|i| {
            let start = offsets[i] as usize;
            let end = offsets[i + 1] as usize;
            // SAFETY: the JS wrapper produces these bytes via `TextEncoder`,
            // which always emits well-formed UTF-8. Skipping the
            // `from_utf8` validation step matches the NAPI raw entry and
            // keeps the per-item conversion branch-free.
            BatchItem {
                source_text: unsafe { std::str::from_utf8_unchecked(&concat[start..end]) },
                base_offset: base_offsets[i],
            }
        })
        .collect();

    let result = parse_batch_to_bytes(&items, options);

    let diagnostic_messages: Vec<String> = result
        .diagnostics
        .iter()
        .map(|d| d.message.to_string())
        .collect();
    let diagnostic_root_indices: Vec<u32> =
        result.diagnostics.iter().map(|d| d.root_index).collect();

    BatchParseResult {
        bytes: result.binary_bytes.into_boxed_slice(),
        diagnostic_messages,
        diagnostic_root_indices,
    }
}
