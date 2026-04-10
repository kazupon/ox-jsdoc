// @author kazuya kawaguchi (a.k.a. kazupon)
// @license MIT
//

mod checkpoint;
mod context;
mod diagnostics;
mod scanner;

use oxc_allocator::{Allocator, Box as ArenaBox};
use oxc_diagnostics::OxcDiagnostic;

use crate::ast::JSDocComment;

pub use checkpoint::{Checkpoint, FenceState, QuoteKind};
pub use context::ParserContext;
pub use diagnostics::{ParserDiagnosticKind, diagnostic};

/// Parser feature switches.
#[derive(Debug, Clone, Copy)]
pub struct ParseOptions {
    /// Treat fenced code blocks as literal text so `@tags` inside examples do
    /// not start new block tag sections.
    pub fence_aware: bool,
    /// Reserved for future inline-code handling.
    pub inline_code_aware: bool,
}

impl Default for ParseOptions {
    fn default() -> Self {
        Self {
            fence_aware: true,
            inline_code_aware: false,
        }
    }
}

/// Parser result containing either an AST or structural diagnostics.
#[derive(Debug)]
pub struct ParseOutput<'a> {
    /// Parsed AST, absent when the input is not a complete JSDoc block.
    pub comment: Option<ArenaBox<'a, JSDocComment<'a>>>,
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

#[cfg(test)]
mod tests {
    use oxc_allocator::Allocator;
    use oxc_diagnostics::OxcDiagnostic;

    use crate::ast::{BlockTagBody, DescriptionPart, TagValueToken};

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

        let description = comment.description.as_ref().expect("expected description");
        assert_eq!(description.span.start, 14);
        assert_eq!(description.span.end, 17);
        assert_eq!(description.parts.len(), 1);
        match &description.parts[0] {
            DescriptionPart::Text(text) => {
                assert_eq!(text.span.start, 14);
                assert_eq!(text.span.end, 16);
                assert_eq!(text.value, "ok");
            }
            _ => panic!("expected text description"),
        }
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
        let description = comment.description.as_ref().expect("expected description");
        assert_eq!(description.parts.len(), 1);
        match &description.parts[0] {
            DescriptionPart::Text(text) => assert_eq!(text.value, "Find a user."),
            _ => panic!("expected text part"),
        }
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
        let description = comment.description.as_ref().expect("expected description");
        assert_eq!(description.parts.len(), 3);
        match &description.parts[1] {
            DescriptionPart::InlineTag(tag) => {
                assert_eq!(tag.tag_name.value, "link");
                assert_eq!(
                    tag.body.as_ref().expect("expected inline tag body").raw,
                    "UserService"
                );
            }
            _ => panic!("expected inline tag part"),
        }
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
        assert_eq!(tag.tag_name.value, "param");
        assert_eq!(
            tag.raw_body.as_ref().expect("expected raw body").value,
            "{string} id - The user ID"
        );

        match tag.body.as_ref().expect("expected block tag body").as_ref() {
            BlockTagBody::Generic(body) => {
                assert_eq!(
                    body.type_expression.as_ref().expect("expected type").raw,
                    "string"
                );
                match body.value.as_ref().expect("expected value").as_ref() {
                    TagValueToken::Parameter(parameter) => {
                        assert_eq!(parameter.path.raw, "id");
                        assert!(!parameter.optional);
                    }
                    _ => panic!("expected parameter value"),
                }
                let description = body.description.as_ref().expect("expected description");
                match &description.parts[0] {
                    DescriptionPart::Text(text) => assert_eq!(text.value, "The user ID"),
                    _ => panic!("expected text description"),
                }
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
        assert_eq!(comment.tags[0].tag_name.value, "param");
        assert_eq!(comment.tags[1].tag_name.value, "returns");
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
        let description = comment.description.as_ref().expect("expected description");
        assert_eq!(description.parts.len(), 2);
        match &description.parts[0] {
            DescriptionPart::Text(text) => assert_eq!(text.value, "See "),
            _ => panic!("expected leading text"),
        }
        match &description.parts[1] {
            DescriptionPart::Text(text) => {
                assert_eq!(text.value, "{@link UserService for details.")
            }
            _ => panic!("expected text fallback"),
        }
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
        assert_eq!(tag.tag_name.value, "param");
        assert_eq!(
            tag.raw_body.as_ref().expect("expected raw body").value,
            "{Object.<string, number> options"
        );
        match tag.body.as_ref().expect("expected body").as_ref() {
            BlockTagBody::Generic(body) => {
                assert!(body.type_expression.is_none());
                match body
                    .value
                    .as_ref()
                    .expect("expected recovered value")
                    .as_ref()
                {
                    TagValueToken::NamePath(name_path) => {
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
        assert_eq!(comment.tags[0].tag_name.value, "example");
        assert_eq!(comment.tags[1].tag_name.value, "returns");
    }
}
