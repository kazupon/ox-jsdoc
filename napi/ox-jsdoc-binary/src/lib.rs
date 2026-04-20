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

use napi::bindgen_prelude::Uint8Array;
use napi_derive::napi;
use oxc_allocator::Allocator;

use ox_jsdoc_binary::parser::{parse, type_data::ParseMode, ParseOptions};

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
    let arena = Allocator::default();
    let opts = convert_options(options);
    let result = parse(&arena, source_text.as_str(), opts);

    let diagnostics = result
        .diagnostics
        .iter()
        .map(|d| JsDiagnostic {
            message: d.message.to_string(),
        })
        .collect();

    // `binary_bytes` is arena-allocated — we copy into a Vec<u8> so the
    // resulting NAPI Buffer owns its memory and the arena can be dropped.
    let bytes = result.binary_bytes.to_vec();

    JsParseResult {
        buffer: Uint8Array::from(bytes),
        diagnostics,
    }
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
