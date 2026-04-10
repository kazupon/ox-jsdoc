// @author kazuya kawaguchi (a.k.a. kazupon)
// @license MIT
//

use oxc_diagnostics::OxcDiagnostic;

/// Parser-level recovery and structural errors.
///
/// These diagnostics describe malformed comment syntax. Tag-policy validation
/// lives in `validator`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParserDiagnosticKind {
    NotAJSDocBlock,
    UnclosedBlockComment,
    SpanOverflow,
    UnclosedInlineTag,
    UnclosedTypeExpression,
    UnclosedFence,
    InvalidTagStart,
    InvalidInlineTagStart,
}

/// Build an `oxc_diagnostics` error for parser diagnostics.
pub fn diagnostic(kind: ParserDiagnosticKind) -> OxcDiagnostic {
    let message = match kind {
        ParserDiagnosticKind::NotAJSDocBlock => "input is not a JSDoc block comment",
        ParserDiagnosticKind::UnclosedBlockComment => "JSDoc block comment is not closed",
        ParserDiagnosticKind::SpanOverflow => "JSDoc comment span exceeds u32 byte offset range",
        ParserDiagnosticKind::UnclosedInlineTag => "inline tag is not closed",
        ParserDiagnosticKind::UnclosedTypeExpression => "type expression is not closed",
        ParserDiagnosticKind::UnclosedFence => "fenced code block is not closed",
        ParserDiagnosticKind::InvalidTagStart => "invalid block tag start",
        ParserDiagnosticKind::InvalidInlineTagStart => "invalid inline tag start",
    };

    OxcDiagnostic::error(message)
}
