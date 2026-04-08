// @author kazuya kawaguchi (a.k.a. kazupon)
// @license MIT
//

use oxc_allocator::{Allocator, Box as ArenaBox, Vec as ArenaVec};
use oxc_diagnostics::OxcDiagnostic;
use oxc_span::Span;

use crate::ast::JSDocComment;

use super::{
    Checkpoint, FenceState, ParseOptions, ParseOutput, ParserDiagnosticKind, QuoteKind, diagnostic,
    scanner,
};

pub struct ParserContext<'a> {
    pub(crate) allocator: &'a Allocator,
    pub(crate) source_text: &'a str,
    pub(crate) base_offset: u32,
    pub(crate) offset: u32,
    pub(crate) _options: ParseOptions,
    pub(crate) diagnostics: Vec<OxcDiagnostic>,
    pub(crate) brace_depth: u16,
    pub(crate) bracket_depth: u16,
    pub(crate) paren_depth: u16,
    pub(crate) quote: Option<QuoteKind>,
    pub(crate) fence: Option<FenceState>,
}

impl<'a> ParserContext<'a> {
    pub fn new(
        allocator: &'a Allocator,
        source_text: &'a str,
        base_offset: u32,
        options: ParseOptions,
    ) -> Self {
        Self {
            allocator,
            source_text,
            base_offset,
            offset: 0,
            _options: options,
            diagnostics: Vec::new(),
            brace_depth: 0,
            bracket_depth: 0,
            paren_depth: 0,
            quote: None,
            fence: None,
        }
    }

    #[must_use]
    pub fn checkpoint(&self) -> Checkpoint {
        Checkpoint {
            offset: self.offset,
            brace_depth: self.brace_depth,
            bracket_depth: self.bracket_depth,
            paren_depth: self.paren_depth,
            quote: self.quote,
            fence: self.fence,
            diagnostics_len: self.diagnostics.len(),
        }
    }

    pub fn rewind(&mut self, checkpoint: Checkpoint) {
        self.offset = checkpoint.offset;
        self.brace_depth = checkpoint.brace_depth;
        self.bracket_depth = checkpoint.bracket_depth;
        self.paren_depth = checkpoint.paren_depth;
        self.quote = checkpoint.quote;
        self.fence = checkpoint.fence;
        self.diagnostics.truncate(checkpoint.diagnostics_len);
    }

    pub fn parse_comment(mut self) -> ParseOutput<'a> {
        let Some(end) = self.absolute_end() else {
            self.diagnostics
                .push(diagnostic(ParserDiagnosticKind::SpanOverflow));
            return ParseOutput {
                comment: None,
                diagnostics: self.diagnostics,
            };
        };

        if !scanner::is_jsdoc_block(self.source_text) {
            self.diagnostics
                .push(diagnostic(ParserDiagnosticKind::NotAJSDocBlock));
            return ParseOutput {
                comment: None,
                diagnostics: self.diagnostics,
            };
        }

        if !scanner::has_closing_block(self.source_text) {
            self.diagnostics
                .push(diagnostic(ParserDiagnosticKind::UnclosedBlockComment));
            return ParseOutput {
                comment: None,
                diagnostics: self.diagnostics,
            };
        }

        let comment = ArenaBox::new_in(
            JSDocComment {
                span: Span::new(self.base_offset, end),
                description: None,
                tags: ArenaVec::new_in(self.allocator),
            },
            self.allocator,
        );

        ParseOutput {
            comment: Some(comment),
            diagnostics: self.diagnostics,
        }
    }

    fn absolute_end(&self) -> Option<u32> {
        let len = u32::try_from(self.source_text.len()).ok()?;
        self.base_offset.checked_add(len)
    }
}
