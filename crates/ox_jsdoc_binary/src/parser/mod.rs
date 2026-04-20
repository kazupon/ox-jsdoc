// @author kazuya kawaguchi (a.k.a. kazupon)
// @license MIT
//

//! Public parser entry points.
//!
//! See `design/007-binary-ast/rust-impl.md#parser-integrated-binary-writer`
//! and `design/007-binary-ast/batch-processing.md` for the design.
//!
//! The parser implements **approach c-1**: it walks the JSDoc source text
//! once, and for every recognised node it invokes a `write_*` helper from
//! [`super::writer`] that appends bytes directly into the arena-backed
//! Binary AST buffer. There is no intermediate typed AST.
//!
//! Phase 1.2a — port-in-progress.
//!
//! `scanner` / `checkpoint` / `diagnostics` are verbatim ports from the
//! typed-AST parser; they have no AST dependency. The structural parser
//! (`context`) and the type expression parser (`type_parse`) land in
//! follow-up commits inside this same Phase.

pub mod checkpoint;
pub mod context;
pub mod diagnostics;
pub mod lexer;
pub mod precedence;
pub mod scanner;
pub mod token;
pub mod type_data;
pub mod type_emit;
pub mod type_parse;

pub use checkpoint::{Checkpoint, FenceState, QuoteKind};
pub use context::{
    emit_block, parse_block_into_data, InlineTagFormatData, ParsedBlock, ParsedDiagnostic,
    ParserContext,
};
pub use diagnostics::{
    parser_diagnostic_message, type_diagnostic_message, ParserDiagnosticKind, TypeDiagnosticKind,
};

use oxc_allocator::{Allocator, Vec as ArenaVec};
use oxc_span::Span;

use crate::decoder::nodes::comment_ast::LazyJsdocBlock;
use crate::decoder::source_file::LazySourceFile;

/// Options controlling parser behaviour.
///
/// Most ox-jsdoc users will leave every field at its [`Default`] value; the
/// fields exist so binding code can flip `compat_mode` for jsdoccomment
/// compatibility (see `design/007-binary-ast/encoding.md`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ParseOptions {
    /// When `true`, the writer also emits the jsdoccomment-compat extension
    /// region on `JsdocBlock` / `JsdocTag` / `JsdocDescriptionLine` /
    /// `JsdocTypeLine`. Sets `Header.flags` bit 0.
    pub compat_mode: bool,
    /// Original-file absolute byte offset of `source`. Stored on the root
    /// index entry so JS-side decoders can rebuild absolute ranges
    /// (`base_offset + pos`).
    ///
    /// Default `0` is correct when the comment is parsed in isolation.
    pub base_offset: u32,
    /// Treat fenced code blocks as literal text so `@tags` inside examples
    /// do not start new block tag sections.
    pub fence_aware: bool,
    /// Enable type expression parsing for `{...}` in tags. When `false`,
    /// the `parsedType` slot is always omitted (zero cost).
    pub parse_types: bool,
    /// Parse mode for the type expression sub-parser. Only used when
    /// `parse_types` is `true`. Defaults to [`type_data::ParseMode::Jsdoc`].
    pub type_parse_mode: type_data::ParseMode,
}

impl Default for ParseOptions {
    fn default() -> Self {
        Self {
            compat_mode: false,
            base_offset: 0,
            fence_aware: true,
            parse_types: false,
            type_parse_mode: type_data::ParseMode::Jsdoc,
        }
    }
}

/// One parser-emitted diagnostic.
///
/// Arena-allocated so it can live alongside the Binary AST bytes without
/// extra heap pressure. The on-wire representation lives in the
/// `Diagnostics` section of the Binary AST, not in this struct directly;
/// this is the convenient Rust-side handle returned by [`parse`].
#[derive(Debug, Clone, Copy)]
pub struct Diagnostic<'arena> {
    /// Human-readable description of the issue.
    pub message: &'arena str,
    /// Source span the diagnostic refers to, when known.
    pub span: Option<Span>,
}

/// Result of [`parse`].
///
/// `binary_bytes` is the canonical Binary AST byte stream (suitable for
/// zero-copy sharing with NAPI/WASM bindings) and `lazy_root` is the
/// matching Rust-side lazy view. Both borrow from the same arena, so they
/// share a lifetime.
#[derive(Debug)]
pub struct ParseResult<'arena> {
    /// Binary AST bytes laid out in the arena. Sized so the whole buffer is
    /// 8-byte aligned for cross-target safety.
    pub binary_bytes: &'arena [u8],
    /// Lazy decoder root for Rust-side walkers. `None` when parsing failed
    /// (the matching `Diagnostics` entry will explain why).
    pub lazy_root: Option<LazyJsdocBlock<'arena>>,
    /// Decoder handle that wraps `binary_bytes` and exposes the cached
    /// Header offsets. Constructed once and shared with every lazy node.
    pub source_file: LazySourceFile<'arena>,
    /// Diagnostics produced while parsing.
    pub diagnostics: ArenaVec<'arena, Diagnostic<'arena>>,
}

/// Parse a single JSDoc block comment into Binary AST.
///
/// `source` is the raw `/** ... */` text exactly as it appears in the
/// surrounding file. `arena` owns every allocation produced by the parser
/// (the byte buffer, intern table, diagnostics) so the caller does not need
/// to free anything explicitly.
///
/// Phase 1.2a structural port: emits all 60 comment-AST kinds and the
/// scalar string fields. The `parsedType` slot is currently always omitted;
/// Phase 1.2a-cont (type_parse port) will enable it.
pub fn parse<'arena>(
    arena: &'arena Allocator,
    source: &'arena str,
    options: ParseOptions,
) -> ParseResult<'arena> {
    use crate::writer::BinaryWriter;

    // Parse with relative spans (base_offset = 0). The root index entry's
    // base_offset captures the absolute position, and the lazy decoder's
    // `range` getter combines them. This avoids double-counting when the
    // caller passes a non-zero base_offset.
    let parser_options = ParseOptions {
        base_offset: 0,
        ..options
    };
    let parsed = context::parse_block_into_data(source, 0, parser_options);

    let mut writer = BinaryWriter::new(arena);
    if options.compat_mode {
        writer.set_compat_mode(true);
    }
    let _ = writer.append_source_text(source);

    let root_node_index = if parsed.is_failure() {
        0
    } else {
        context::emit_block(&mut writer, &parsed).unwrap_or(0)
    };
    writer.push_root(root_node_index, 0, options.base_offset);

    // Diagnostics: writer interns the message and records (root_index=0).
    for diag in parsed.diagnostics() {
        writer.push_diagnostic(0, diag.message());
    }

    let arena_diagnostics: ArenaVec<'arena, Diagnostic<'arena>> = {
        let mut v = ArenaVec::new_in(arena);
        for diag in parsed.diagnostics() {
            v.push(Diagnostic {
                message: arena.alloc_str(diag.message()),
                span: diag.span,
            });
        }
        v
    };

    let bytes_vec = writer.finish();
    let binary_bytes: &'arena [u8] = arena.alloc_slice_copy(&bytes_vec);
    let source_file_owned = LazySourceFile::new(binary_bytes)
        .expect("BinaryWriter::finish() always produces a header-valid buffer");
    let source_file_ref: &'arena LazySourceFile<'arena> = arena.alloc(source_file_owned);

    let lazy_root = if root_node_index == 0 {
        None
    } else {
        source_file_ref.asts().next().flatten()
    };

    ParseResult {
        binary_bytes,
        lazy_root,
        source_file: *source_file_ref,
        diagnostics: arena_diagnostics,
    }
}

/// One input item for [`parse_batch`].
///
/// Mirrors the public `BatchItem` interface in `js-decoder.md` (the JS-side
/// API takes the same shape so the NAPI binding can pass values through
/// with no transformation).
#[derive(Debug, Clone, Copy)]
pub struct BatchItem<'a> {
    /// `/** ... */` source text for this comment.
    pub source_text: &'a str,
    /// Original-file absolute byte offset.
    pub base_offset: u32,
}

/// Result of [`parse_batch`]; carries N roots in a single shared buffer.
///
/// The shape intentionally matches [`ParseResult`] but with a multi-root
/// array — `lazy_roots[i]` is `None` when `items[i]` failed to parse. The
/// matching `Diagnostics` entries (sorted by `root_index` ascending in the
/// Binary AST) explain each failure.
#[derive(Debug)]
pub struct BatchResult<'arena> {
    /// Binary AST bytes shared by all roots.
    pub binary_bytes: &'arena [u8],
    /// One entry per input `BatchItem`; `None` indicates a parse failure.
    pub lazy_roots: ArenaVec<'arena, Option<LazyJsdocBlock<'arena>>>,
    /// Decoder handle that wraps `binary_bytes`.
    pub source_file: LazySourceFile<'arena>,
    /// All diagnostics produced during the batch (`root_index` is implied
    /// by the corresponding entry in the Binary AST `Diagnostics` section).
    pub diagnostics: ArenaVec<'arena, Diagnostic<'arena>>,
}

/// Parse N JSDoc block comments into a single shared Binary AST buffer.
///
/// **Reserved for Phase 2** — see `design/007-binary-ast/phases.md`
/// "Phase 2: Batch support + public encoder API". Phase 1 deliberately
/// implements only the single-comment path so the Phase 1.3 cutover
/// decision is driven by the simpler shape.
pub fn parse_batch<'arena>(
    _arena: &'arena Allocator,
    _items: &[BatchItem<'_>],
    _options: ParseOptions,
) -> BatchResult<'arena> {
    unimplemented!(
        "Phase 2: implement batch parsing per design/007-binary-ast/batch-processing.md"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_options_default_is_non_compat_zero_offset() {
        let opts = ParseOptions::default();
        assert!(!opts.compat_mode);
        assert_eq!(opts.base_offset, 0);
    }

    #[test]
    fn parse_options_is_copy() {
        // Compile-time check: ParseOptions must be Copy so the parser can
        // pass it by value into hot loops without lifetime gymnastics.
        fn assert_copy<T: Copy>() {}
        assert_copy::<ParseOptions>();
    }

    #[test]
    fn diagnostic_is_copy() {
        fn assert_copy<T: Copy>() {}
        assert_copy::<Diagnostic<'static>>();
    }

    #[test]
    fn batch_item_is_copy() {
        fn assert_copy<T: Copy>() {}
        assert_copy::<BatchItem<'static>>();
    }

    #[test]
    fn parse_simple_block_emits_lazy_root() {
        let arena = Allocator::default();
        let result = parse(&arena, "/** ok */", ParseOptions::default());
        assert!(result.diagnostics.is_empty());
        let root = result.lazy_root.expect("root present");
        assert_eq!(root.description(), Some("ok"));
    }

    #[test]
    fn parse_param_tag_round_trips_through_lazy_decoder() {
        let arena = Allocator::default();
        let result = parse(
            &arena,
            "/**\n * @param {string} id - The user ID\n */",
            ParseOptions::default(),
        );
        assert!(result.diagnostics.is_empty());
        let root = result.lazy_root.expect("root present");
        let tags: Vec<_> = root.tags().collect();
        assert_eq!(tags.len(), 1);
        let tag = tags[0];
        assert_eq!(tag.tag().value(), "param");
        assert_eq!(tag.description(), Some("The user ID"));
    }

    #[test]
    fn parse_failure_yields_diagnostic_and_no_root() {
        let arena = Allocator::default();
        let result = parse(&arena, "/* plain */", ParseOptions::default());
        assert!(result.lazy_root.is_none());
        assert_eq!(result.diagnostics.len(), 1);
        assert!(result.diagnostics[0].message.contains("not a JSDoc block"));
    }

    #[test]
    fn parse_with_parsed_type_emits_type_name() {
        use crate::decoder::nodes::type_node::LazyTypeNode;
        let arena = Allocator::default();
        let mut opts = ParseOptions::default();
        opts.parse_types = true;
        let result = parse(
            &arena,
            "/**\n * @param {string} id\n */",
            opts,
        );
        assert!(result.diagnostics.is_empty());
        let root = result.lazy_root.unwrap();
        let tag = root.tags().next().expect("tag present");
        let parsed = tag.parsed_type().expect("parsedType emitted");
        match parsed {
            LazyTypeNode::Name(n) => assert_eq!(n.value(), "string"),
            other => panic!("expected TypeName, got {other:?}"),
        }
    }

    #[test]
    fn parse_with_parsed_type_emits_union() {
        use crate::decoder::nodes::type_node::LazyTypeNode;
        let arena = Allocator::default();
        let mut opts = ParseOptions::default();
        opts.parse_types = true;
        opts.type_parse_mode = crate::parser::type_data::ParseMode::Typescript;
        let result = parse(
            &arena,
            "/**\n * @param {string | number} id\n */",
            opts,
        );
        assert!(result.diagnostics.is_empty());
        let root = result.lazy_root.unwrap();
        let tag = root.tags().next().expect("tag present");
        let parsed = tag.parsed_type().expect("parsedType emitted");
        match parsed {
            LazyTypeNode::Union(u) => assert_eq!(u.elements().count(), 2),
            other => panic!("expected TypeUnion, got {other:?}"),
        }
    }

    #[test]
    fn parse_with_parsed_type_emits_function_type() {
        use crate::decoder::nodes::type_node::LazyTypeNode;
        let arena = Allocator::default();
        let mut opts = ParseOptions::default();
        opts.parse_types = true;
        opts.type_parse_mode = crate::parser::type_data::ParseMode::Jsdoc;
        let result = parse(
            &arena,
            "/**\n * @returns {function(string): number} ok\n */",
            opts,
        );
        assert!(result.diagnostics.is_empty());
        let root = result.lazy_root.unwrap();
        let tag = root.tags().next().expect("tag present");
        let parsed = tag.parsed_type().expect("parsedType emitted");
        assert!(matches!(parsed, LazyTypeNode::Function(_)));
    }

    #[test]
    fn parse_handles_generic_dot_notation() {
        use crate::decoder::nodes::type_node::LazyTypeNode;
        let arena = Allocator::default();
        let mut opts = ParseOptions::default();
        opts.parse_types = true;
        let result = parse(
            &arena,
            "/**\n * @param {Array.<string>} ids\n */",
            opts,
        );
        assert!(result.diagnostics.is_empty());
        let root = result.lazy_root.unwrap();
        let tag = root.tags().next().expect("tag present");
        match tag.parsed_type().expect("parsedType emitted") {
            LazyTypeNode::Generic(g) => {
                assert!(g.dot());
                assert_eq!(g.elements().count(), 1);
            }
            other => panic!("expected TypeGeneric, got {other:?}"),
        }
    }

    #[test]
    fn parse_handles_template_literal_type() {
        use crate::decoder::nodes::type_node::LazyTypeNode;
        let arena = Allocator::default();
        let mut opts = ParseOptions::default();
        opts.parse_types = true;
        opts.type_parse_mode = crate::parser::type_data::ParseMode::Typescript;
        let result = parse(
            &arena,
            "/**\n * @param {`hello-${T}`} value\n */",
            opts,
        );
        assert!(result.diagnostics.is_empty());
        let root = result.lazy_root.unwrap();
        let tag = root.tags().next().expect("tag present");
        assert!(matches!(
            tag.parsed_type().expect("parsedType emitted"),
            LazyTypeNode::TemplateLiteral(_)
        ));
    }

    #[test]
    fn parse_handles_conditional_type() {
        use crate::decoder::nodes::type_node::LazyTypeNode;
        let arena = Allocator::default();
        let mut opts = ParseOptions::default();
        opts.parse_types = true;
        opts.type_parse_mode = crate::parser::type_data::ParseMode::Typescript;
        let result = parse(
            &arena,
            "/**\n * @param {T extends U ? X : Y} v\n */",
            opts,
        );
        assert!(result.diagnostics.is_empty());
        let root = result.lazy_root.unwrap();
        let tag = root.tags().next().expect("tag present");
        assert!(matches!(
            tag.parsed_type().expect("parsedType emitted"),
            LazyTypeNode::Conditional(_)
        ));
    }
}
