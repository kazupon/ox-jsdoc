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
}
