// @author kazuya kawaguchi (a.k.a. kazupon)
// @license MIT
//

//! NAPI binding for the binary AST flavor of ox-jsdoc.
//!
//! Returns the encoded Binary AST as a NAPI Buffer that the JS-side
//! `@ox-jsdoc/decoder` package wraps with `RemoteSourceFile`. Diagnostics
//! travel as a separate plain-object array (the binary buffer also carries
//! them in its Diagnostics section, but exposing both lets the JS layer
//! avoid a re-decode for the common case).

use napi::bindgen_prelude::{Uint32Array, Uint8Array};
use napi_derive::napi;

use ox_jsdoc_binary::parser::{
    parse_batch_to_bytes, parse_to_bytes, type_data::ParseMode, BatchItem, ParseOptions,
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
    /// Enable jsdoccomment-compat extension fields. Default: false.
    pub compat_mode: Option<bool>,
    /// Original-file absolute byte offset of `source_text`. Default: 0.
    pub base_offset: Option<u32>,
}

#[napi(object)]
pub struct JsDiagnostic {
    /// Human-readable diagnostic message.
    pub message: String,
}

#[napi(object)]
pub struct JsParseResult {
    /// Encoded Binary AST bytes — pass to `@ox-jsdoc/decoder`'s
    /// `RemoteSourceFile` constructor.
    pub buffer: Uint8Array,
    /// Parser diagnostics (also embedded in the binary buffer's
    /// Diagnostics section, surfaced here for convenience).
    pub diagnostics: Vec<JsDiagnostic>,
}

/// Parse a complete `/** ... */` JSDoc block and return the Binary AST.
#[napi]
pub fn parse_jsdoc(source_text: String, options: Option<JsParseOptions>) -> JsParseResult {
    let opts = convert_options(options);
    let result = parse_to_bytes(source_text.as_str(), opts);

    let diagnostics = result
        .diagnostics
        .iter()
        .map(|d| JsDiagnostic {
            message: d.message.to_string(),
        })
        .collect();

    // `parse_to_bytes` returns a heap-owned `Vec<u8>` (no arena copy on the
    // Rust side). Moving it into `Uint8Array::from` transfers ownership to
    // NAPI without an additional memcpy.
    JsParseResult {
        buffer: Uint8Array::from(result.binary_bytes),
        diagnostics,
    }
}

#[napi(object)]
pub struct JsBatchItem {
    /// `/** ... */` source text for this comment.
    pub source_text: String,
    /// Original-file absolute byte offset. Default: 0.
    pub base_offset: Option<u32>,
}

#[napi(object)]
pub struct JsBatchDiagnostic {
    /// Human-readable diagnostic message.
    pub message: String,
    /// Index of the input item this diagnostic belongs to (`0..items.len()`).
    pub root_index: u32,
}

#[napi(object)]
pub struct JsBatchParseResult {
    /// Encoded Binary AST bytes — pass to `@ox-jsdoc/decoder`'s
    /// `RemoteSourceFile` constructor; one buffer carries N roots.
    pub buffer: Uint8Array,
    /// Parser diagnostics for the entire batch (input order). Use
    /// `root_index` to attribute each diagnostic back to a `BatchItem`.
    pub diagnostics: Vec<JsBatchDiagnostic>,
}

/// Parse N JSDoc block comments into a single shared Binary AST buffer.
///
/// The returned buffer carries N roots side-by-side; the JS-side decoder
/// exposes them via `RemoteSourceFile.asts`. Strings recur across comments
/// (`*`, `*/`, common tag names) are interned once for the whole batch.
#[napi]
pub fn parse_jsdoc_batch(
    items: Vec<JsBatchItem>,
    options: Option<JsParseOptions>,
) -> JsBatchParseResult {
    let opts = convert_options(options);

    // BatchItem borrows `source_text: &str` from the input Vec, so we need
    // to project the JS-side Strings into a parallel Vec<BatchItem>.
    let batch_items: Vec<BatchItem<'_>> = items
        .iter()
        .map(|i| BatchItem {
            source_text: i.source_text.as_str(),
            base_offset: i.base_offset.unwrap_or(0),
        })
        .collect();

    let result = parse_batch_to_bytes(&batch_items, opts);

    let diagnostics = result
        .diagnostics
        .iter()
        .map(|d| JsBatchDiagnostic {
            message: d.message.to_string(),
            root_index: d.root_index,
        })
        .collect();

    JsBatchParseResult {
        buffer: Uint8Array::from(result.binary_bytes),
        diagnostics,
    }
}

/// Parse N JSDoc block comments where the JS-side has already concatenated
/// every `source_text` into a single UTF-8 byte buffer.
///
/// This avoids the per-item `Vec<JsBatchItem>` auto-conversion that
/// dominates the [`parse_jsdoc_batch`] NAPI call (~213 µs / ~30% of the
/// full call for the 226-comment fixture). Three NAPI handles replace
/// 226 × {object, string, number} conversions:
///
/// - `concat`: every `source_text` UTF-8 byte concatenated, no separators.
/// - `offsets`: length `N + 1`. `concat[offsets[i]..offsets[i+1]]` is the
///   bytes for input item `i`.
/// - `base_offsets`: length `N`. Per-item `base_offset`.
///
/// The JS wrapper (`parseBatch` in `index.js`) builds those views via
/// `TextEncoder`. Callers who need the original ergonomic API can keep
/// using [`parse_jsdoc_batch`].
#[napi]
pub fn parse_jsdoc_batch_raw(
    concat: Uint8Array,
    offsets: Uint32Array,
    base_offsets: Uint32Array,
    options: Option<JsParseOptions>,
) -> JsBatchParseResult {
    let opts = convert_options(options);
    let bytes: &[u8] = concat.as_ref();
    let offsets_slice: &[u32] = offsets.as_ref();
    let base_offsets_slice: &[u32] = base_offsets.as_ref();
    let n = base_offsets_slice.len();

    let batch_items: Vec<BatchItem<'_>> = (0..n)
        .map(|i| {
            let start = offsets_slice[i] as usize;
            let end = offsets_slice[i + 1] as usize;
            // SAFETY: the JS wrapper produces these bytes via `TextEncoder`,
            // which RFC 8259 mandates emit valid UTF-8. Skipping the
            // `from_utf8` validation cuts ~10 µs for a 30 KB input batch.
            BatchItem {
                source_text: unsafe { std::str::from_utf8_unchecked(&bytes[start..end]) },
                base_offset: base_offsets_slice[i],
            }
        })
        .collect();

    let result = parse_batch_to_bytes(&batch_items, opts);

    let diagnostics = result
        .diagnostics
        .iter()
        .map(|d| JsBatchDiagnostic {
            message: d.message.to_string(),
            root_index: d.root_index,
        })
        .collect();

    JsBatchParseResult {
        buffer: Uint8Array::from(result.binary_bytes),
        diagnostics,
    }
}

// ---------------------------------------------------------------------------
// Diagnostic no-op entry points (used by the NAPI overhead profiler only).
// ---------------------------------------------------------------------------

/// Diagnostic: pay only the input-side marshalling cost (Vec<JsBatchItem>
/// auto-conversion). Returns the item count so the call cannot be DCE'd.
#[doc(hidden)]
#[napi]
pub fn napi_marshalling_in_only(items: Vec<JsBatchItem>) -> u32 {
    items.len() as u32
}

/// Diagnostic: pay only the output-side marshalling cost (Vec<u8> →
/// `Uint8Array` ownership transfer). Allocates a zero-filled buffer of
/// `size` bytes and hands it to NAPI.
#[doc(hidden)]
#[napi]
pub fn napi_marshalling_out_only(size: u32) -> Uint8Array {
    Uint8Array::from(vec![0u8; size as usize])
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
        compat_mode: options.compat_mode.unwrap_or(false),
        base_offset: options.base_offset.unwrap_or(0),
    }
}
