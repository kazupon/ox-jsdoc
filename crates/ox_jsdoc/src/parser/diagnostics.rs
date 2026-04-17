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

/// Type parser diagnostics.
///
/// These diagnostics describe malformed type expressions inside `{...}`.
/// Consolidated in the same file as `ParserDiagnosticKind` so both parsers
/// share one diagnostics module.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TypeDiagnosticKind {
    /// No prefix parslet matched the current token.
    NoParsletFound,
    /// Expected a specific token but found something else.
    ExpectedToken,
    /// Generic type parameter list `<...>` is not closed.
    UnclosedGeneric,
    /// Parenthesized type `(...)` is not closed.
    UnclosedParenthesis,
    /// Tuple type `[...]` is not closed.
    UnclosedTuple,
    /// Object type `{...}` is not closed.
    UnclosedObject,
    /// Template literal type is not closed.
    UnclosedTemplateLiteral,
    /// General invalid type expression.
    InvalidTypeExpression,
    /// Unexpected token after a complete type expression.
    EarlyEndOfParse,
}

/// Build an `oxc_diagnostics` error for type parser diagnostics.
pub fn type_diagnostic(kind: TypeDiagnosticKind) -> OxcDiagnostic {
    let message = match kind {
        TypeDiagnosticKind::NoParsletFound => "unexpected token in type expression",
        TypeDiagnosticKind::ExpectedToken => "expected token in type expression",
        TypeDiagnosticKind::UnclosedGeneric => "generic type parameter list is not closed",
        TypeDiagnosticKind::UnclosedParenthesis => "parenthesized type is not closed",
        TypeDiagnosticKind::UnclosedTuple => "tuple type is not closed",
        TypeDiagnosticKind::UnclosedObject => "object type is not closed",
        TypeDiagnosticKind::UnclosedTemplateLiteral => "template literal type is not closed",
        TypeDiagnosticKind::InvalidTypeExpression => "invalid type expression",
        TypeDiagnosticKind::EarlyEndOfParse => "unexpected token after type expression",
    };

    OxcDiagnostic::error(message)
}
