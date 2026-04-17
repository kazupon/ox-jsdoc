// @author kazuya kawaguchi (a.k.a. kazupon)
// @license MIT
//

mod checkpoint;
mod context;
mod diagnostics;
mod scanner;
mod type_parse;

use oxc_allocator::{Allocator, Box as ArenaBox};
use oxc_diagnostics::OxcDiagnostic;

use crate::ast::JsdocBlock;
use crate::type_parser::ast::ParseMode;

pub use checkpoint::{Checkpoint, FenceState, QuoteKind};
pub use context::ParserContext;
pub use diagnostics::{ParserDiagnosticKind, TypeDiagnosticKind, diagnostic, type_diagnostic};

/// Parser feature switches.
#[derive(Debug, Clone, Copy)]
pub struct ParseOptions {
    /// Treat fenced code blocks as literal text so `@tags` inside examples do
    /// not start new block tag sections.
    pub fence_aware: bool,
    /// Reserved for future inline-code handling.
    pub inline_code_aware: bool,
    /// Enable type expression parsing for `{...}` in tags.
    /// When `false`, `parsed_type` is always `None` (zero cost).
    pub parse_types: bool,
    /// Parse mode for type expressions. Only used when `parse_types` is `true`.
    /// Defaults to `ParseMode::Jsdoc`.
    pub type_parse_mode: ParseMode,
}

impl Default for ParseOptions {
    fn default() -> Self {
        Self {
            fence_aware: true,
            inline_code_aware: false,
            parse_types: false,
            type_parse_mode: ParseMode::Jsdoc,
        }
    }
}

/// Parser result containing either an AST or structural diagnostics.
#[derive(Debug)]
pub struct ParseOutput<'a> {
    /// Parsed AST, absent when the input is not a complete JSDoc block.
    pub comment: Option<ArenaBox<'a, JsdocBlock<'a>>>,
    /// Parser diagnostics collected during structural parsing and recovery.
    pub diagnostics: Vec<OxcDiagnostic>,
}

/// Parse a complete `/** ... */` JSDoc block.
///
/// `base_offset` lets callers parse a slice while preserving byte spans in the
/// original source file.
pub fn parse_comment<'a>(
    allocator: &'a Allocator,
    source_text: &'a str,
    base_offset: u32,
    options: ParseOptions,
) -> ParseOutput<'a> {
    ParserContext::new(allocator, source_text, base_offset, options).parse_comment()
}

/// Parse a standalone type expression (e.g. `string | number`).
///
/// This is a lightweight entry point for type-only parsing without comment
/// parsing overhead. Used by benchmarks and consumers that already have
/// extracted type text.
pub fn parse_type<'a>(
    allocator: &'a Allocator,
    type_text: &'a str,
    base_offset: u32,
    mode: crate::type_parser::ast::ParseMode,
) -> ParseTypeOutput<'a> {
    let mut ctx = ParserContext::new(allocator, "/** */", 0, ParseOptions::default());
    let node = ctx.parse_type_expression(type_text, base_offset, mode);
    ParseTypeOutput {
        node,
        diagnostics: ctx.diagnostics,
    }
}

/// Result of standalone type expression parsing.
#[derive(Debug)]
pub struct ParseTypeOutput<'a> {
    /// Parsed type AST node, absent when the input is invalid.
    pub node: Option<ArenaBox<'a, crate::type_parser::ast::TypeNode<'a>>>,
    /// Diagnostics collected during type parsing.
    pub diagnostics: Vec<OxcDiagnostic>,
}

#[cfg(test)]
mod tests {
    use oxc_allocator::Allocator;
    use oxc_diagnostics::OxcDiagnostic;

    use crate::ast::{JsdocTagBody, JsdocTagValue};

    use super::{ParseOptions, parse_comment};

    fn assert_single_diagnostic_contains(diagnostics: &[OxcDiagnostic], expected: &str) {
        assert_eq!(diagnostics.len(), 1);
        assert!(
            diagnostics[0].to_string().contains(expected),
            "expected diagnostic to contain `{expected}`, got `{}`",
            diagnostics[0]
        );
    }

    #[test]
    fn parses_a_bounded_jsdoc_block() {
        let allocator = Allocator::default();
        let output = parse_comment(&allocator, "/** ok */", 10, ParseOptions::default());

        assert!(output.diagnostics.is_empty());
        let comment = output.comment.expect("expected a comment AST");
        assert_eq!(comment.span.start, 10);
        assert_eq!(comment.span.end, 19);
        assert_eq!(comment.tags.len(), 0);

        assert_eq!(comment.description, Some("ok"));
        assert_eq!(comment.description_lines.len(), 1);
        assert_eq!(comment.description_lines[0].span.start, 14);
        assert_eq!(comment.description_lines[0].span.end, 17);
        assert_eq!(comment.description_lines[0].description, "ok");
    }

    #[test]
    fn rejects_non_jsdoc_input() {
        let allocator = Allocator::default();
        let output = parse_comment(&allocator, "/* plain */", 0, ParseOptions::default());

        assert!(output.comment.is_none());
        assert_single_diagnostic_contains(&output.diagnostics, "input is not a JSDoc block");
    }

    #[test]
    fn rejects_unclosed_block_comments() {
        let allocator = Allocator::default();
        let output = parse_comment(&allocator, "/** unclosed", 0, ParseOptions::default());

        assert!(output.comment.is_none());
        assert_single_diagnostic_contains(&output.diagnostics, "JSDoc block comment is not closed");
    }

    #[test]
    fn parses_top_level_description() {
        let allocator = Allocator::default();
        let output = parse_comment(
            &allocator,
            "/**\n * Find a user.\n */",
            0,
            ParseOptions::default(),
        );

        assert!(output.diagnostics.is_empty());
        let comment = output.comment.expect("expected a comment AST");
        assert_eq!(comment.description, Some("Find a user."));
        // Empty lines are skipped in description_lines; only content lines remain
        assert_eq!(comment.description_lines.len(), 1);
        assert_eq!(comment.description_lines[0].description, "Find a user.");
    }

    #[test]
    fn parses_inline_tag_inside_description() {
        let allocator = Allocator::default();
        let output = parse_comment(
            &allocator,
            "/** See {@link UserService} for details. */",
            0,
            ParseOptions::default(),
        );

        assert!(output.diagnostics.is_empty());
        let comment = output.comment.expect("expected a comment AST");
        assert_eq!(
            comment.description,
            Some("See {@link UserService} for details.")
        );
        assert_eq!(comment.inline_tags.len(), 1);
        assert_eq!(comment.inline_tags[0].tag.value, "link");
        assert_eq!(comment.inline_tags[0].namepath_or_url, Some("UserService"));
    }

    #[test]
    fn parses_param_tag_with_type_value_and_description() {
        let allocator = Allocator::default();
        let output = parse_comment(
            &allocator,
            "/**\n * @param {string} id - The user ID\n */",
            0,
            ParseOptions::default(),
        );

        assert!(output.diagnostics.is_empty());
        let comment = output.comment.expect("expected a comment AST");
        assert_eq!(comment.tags.len(), 1);
        let tag = &comment.tags[0];
        assert_eq!(tag.tag.value, "param");
        assert_eq!(
            tag.raw_body.expect("expected raw body"),
            "{string} id - The user ID"
        );
        assert_eq!(tag.raw_type.expect("expected raw type").raw, "string");
        assert_eq!(tag.name.expect("expected name").raw, "id");
        assert_eq!(tag.description, Some("The user ID"));

        match tag.body.as_ref().expect("expected block tag body").as_ref() {
            JsdocTagBody::Generic(body) => {
                assert_eq!(body.type_source.expect("expected type").raw, "string");
                match body.value.as_ref().expect("expected value") {
                    JsdocTagValue::Parameter(parameter) => {
                        assert_eq!(parameter.path, "id");
                        assert!(!parameter.optional);
                    }
                    _ => panic!("expected parameter value"),
                }
                assert_eq!(body.description, Some("The user ID"));
            }
            _ => panic!("expected generic body"),
        }
    }

    #[test]
    fn parses_multiple_tags() {
        let allocator = Allocator::default();
        let output = parse_comment(
            &allocator,
            "/**\n * @param {string} id\n * @returns {User}\n */",
            0,
            ParseOptions::default(),
        );

        assert!(output.diagnostics.is_empty());
        let comment = output.comment.expect("expected a comment AST");
        assert_eq!(comment.tags.len(), 2);
        assert_eq!(comment.tags[0].tag.value, "param");
        assert_eq!(comment.tags[1].tag.value, "returns");
    }

    #[test]
    fn recovers_unclosed_inline_tag_as_text() {
        let allocator = Allocator::default();
        let output = parse_comment(
            &allocator,
            "/** See {@link UserService for details. */",
            0,
            ParseOptions::default(),
        );

        assert_single_diagnostic_contains(&output.diagnostics, "inline tag is not closed");
        let comment = output.comment.expect("expected a comment AST");
        assert_eq!(
            comment.description,
            Some("See {@link UserService for details.")
        );
        assert!(comment.inline_tags.is_empty());
    }

    #[test]
    fn recovers_unclosed_type_expression() {
        let allocator = Allocator::default();
        let output = parse_comment(
            &allocator,
            "/** @param {Object.<string, number> options */",
            0,
            ParseOptions::default(),
        );

        assert_single_diagnostic_contains(&output.diagnostics, "type expression is not closed");
        let comment = output.comment.expect("expected a comment AST");
        assert!(comment.description.is_none());
        assert_eq!(comment.tags.len(), 1);
        let tag = &comment.tags[0];
        assert_eq!(tag.tag.value, "param");
        assert_eq!(
            tag.raw_body.expect("expected raw body"),
            "{Object.<string, number> options"
        );
        match tag.body.as_ref().expect("expected body").as_ref() {
            JsdocTagBody::Generic(body) => {
                assert!(body.type_source.is_none());
                match body.value.as_ref().expect("expected recovered value") {
                    JsdocTagValue::Namepath(name_path) => {
                        assert_eq!(name_path.raw, "{Object.<string,");
                    }
                    _ => panic!("expected name path recovered value"),
                }
            }
            _ => panic!("expected generic body"),
        }
    }

    #[test]
    fn suppresses_block_tags_inside_fenced_code() {
        let allocator = Allocator::default();
        let output = parse_comment(
            &allocator,
            "/**\n * @example\n * ```ts\n * @decorator()\n * ```\n * @returns {void}\n */",
            0,
            ParseOptions::default(),
        );

        assert!(output.diagnostics.is_empty());
        let comment = output.comment.expect("expected a comment AST");
        assert_eq!(comment.tags.len(), 2);
        assert_eq!(comment.tags[0].tag.value, "example");
        assert_eq!(comment.tags[1].tag.value, "returns");
    }

    #[test]
    fn parsed_type_is_none_when_parse_types_disabled() {
        let allocator = Allocator::default();
        let output = parse_comment(
            &allocator,
            "/**\n * @param {string} id\n */",
            0,
            ParseOptions::default(), // parse_types: false
        );

        assert!(output.diagnostics.is_empty());
        let comment = output.comment.expect("expected a comment AST");
        assert_eq!(comment.tags.len(), 1);
        assert!(comment.tags[0].raw_type.is_some());
        assert!(comment.tags[0].parsed_type.is_none());
    }

    #[test]
    fn parsed_type_is_populated_when_parse_types_enabled() {
        use crate::type_parser::ast::ParseMode;

        let allocator = Allocator::default();
        let output = parse_comment(
            &allocator,
            "/**\n * @param {string} id\n */",
            0,
            ParseOptions {
                parse_types: true,
                type_parse_mode: ParseMode::Jsdoc,
                ..ParseOptions::default()
            },
        );

        assert!(output.diagnostics.is_empty());
        let comment = output.comment.expect("expected a comment AST");
        assert_eq!(comment.tags.len(), 1);
        assert!(comment.tags[0].raw_type.is_some());
        assert!(comment.tags[0].parsed_type.is_some());
    }

    #[test]
    fn parsed_type_handles_union_type() {
        use crate::type_parser::ast::ParseMode;

        let allocator = Allocator::default();
        let output = parse_comment(
            &allocator,
            "/**\n * @param {string | number} id\n */",
            0,
            ParseOptions {
                parse_types: true,
                type_parse_mode: ParseMode::Typescript,
                ..ParseOptions::default()
            },
        );

        assert!(output.diagnostics.is_empty());
        let comment = output.comment.expect("expected a comment AST");
        assert_eq!(comment.tags.len(), 1);
        assert!(comment.tags[0].parsed_type.is_some());
    }
}
