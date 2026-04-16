// @author kazuya kawaguchi (a.k.a. kazupon)
// @license MIT
//

//! NAPI binding for ox-jsdoc.

use napi_derive::napi;
use oxc_allocator::Allocator;

use ox_jsdoc::{ParseOptions, parse_comment, serialize_comment_json};

#[napi(object)]
#[derive(Default)]
pub struct JsParseOptions {
    /// Suppress tag recognition inside fenced code blocks. Default: true.
    pub fence_aware: Option<bool>,
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

fn convert_options(options: Option<JsParseOptions>) -> ParseOptions {
    let options = options.unwrap_or_default();
    ParseOptions {
        fence_aware: options.fence_aware.unwrap_or(true),
        ..ParseOptions::default()
    }
}
