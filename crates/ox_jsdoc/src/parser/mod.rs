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

#[derive(Debug, Clone, Copy)]
pub struct ParseOptions {
    pub fence_aware: bool,
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

#[derive(Debug)]
pub struct ParseOutput<'a> {
    pub comment: Option<ArenaBox<'a, JSDocComment<'a>>>,
    pub diagnostics: Vec<OxcDiagnostic>,
}

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

    use crate::ast::{BlockTagBody, DescriptionPart, TagValueToken};

    use super::{ParseOptions, parse_comment};

    #[test]
    fn parses_a_bounded_jsdoc_block() {
        let allocator = Allocator::default();
        let output = parse_comment(&allocator, "/** ok */", 10, ParseOptions::default());

        assert!(output.diagnostics.is_empty());
        let comment = output.comment.expect("expected a comment AST");
        assert_eq!(comment.span.start, 10);
        assert_eq!(comment.span.end, 19);
    }

    #[test]
    fn rejects_non_jsdoc_input() {
        let allocator = Allocator::default();
        let output = parse_comment(&allocator, "/* plain */", 0, ParseOptions::default());

        assert!(output.comment.is_none());
        assert_eq!(output.diagnostics.len(), 1);
    }

    #[test]
    fn rejects_unclosed_block_comments() {
        let allocator = Allocator::default();
        let output = parse_comment(&allocator, "/** unclosed", 0, ParseOptions::default());

        assert!(output.comment.is_none());
        assert_eq!(output.diagnostics.len(), 1);
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

        assert_eq!(output.diagnostics.len(), 1);
        let comment = output.comment.expect("expected a comment AST");
        let description = comment.description.as_ref().expect("expected description");
        assert_eq!(description.parts.len(), 2);
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

        assert_eq!(output.diagnostics.len(), 1);
        let comment = output.comment.expect("expected a comment AST");
        let tag = &comment.tags[0];
        match tag.body.as_ref().expect("expected body").as_ref() {
            BlockTagBody::Generic(body) => {
                assert!(body.type_expression.is_none());
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
