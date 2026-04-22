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

/// Heap-owned counterpart of [`Diagnostic`] returned by [`parse_to_bytes`].
///
/// `message` is borrowed from the `&'static` table inside
/// [`crate::parser::diagnostics`], so this struct itself owns no heap data.
#[derive(Debug, Clone, Copy)]
pub struct OwnedDiagnostic {
    /// Human-readable description of the issue.
    pub message: &'static str,
    /// Source span the diagnostic refers to, when known.
    pub span: Option<Span>,
}

/// Result of [`parse_to_bytes`] — the bytes-only path used by binding code.
///
/// Unlike [`ParseResult`], `binary_bytes` is a heap-owned `Vec<u8>` so it
/// can move directly into a NAPI `Uint8Array` or a `Box<[u8]>` for WASM
/// without going through an arena copy. Use this when you do not need the
/// Rust-side `LazySourceFile` / `LazyJsdocBlock` handles.
#[derive(Debug)]
pub struct ParseBytesResult {
    /// Binary AST bytes — owned, 8-byte aligned by the writer.
    pub binary_bytes: Vec<u8>,
    /// Diagnostics produced while parsing.
    pub diagnostics: Vec<OwnedDiagnostic>,
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
    let parsed = context::parse_block_into_data(arena, source, 0, parser_options);

    let mut writer = BinaryWriter::new(arena);
    if options.compat_mode {
        writer.set_compat_mode(true);
    }
    // Source text is appended after the writer's pre-interned common
    // strings (`StringTableBuilder::new`); the returned offset is what
    // `push_root` needs to record so the decoder can locate the source
    // span via `root[i].source_offset_in_data`.
    let source_offset = writer.append_source_text(source);

    let root_node_index = if parsed.is_failure() {
        0
    } else {
        context::emit_block(&mut writer, &parsed).unwrap_or(0)
    };
    writer.push_root(root_node_index, source_offset, options.base_offset);

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

/// Parse a single JSDoc block comment and return only the Binary AST bytes.
///
/// This is the bytes-only sibling of [`parse`]. Binding code that hands the
/// buffer straight to JS (NAPI `Uint8Array`, WASM `Box<[u8]>`) should call
/// this entry point: it skips the arena copy that [`parse`] performs to
/// satisfy the `&'arena [u8]` lifetime needed by [`LazySourceFile`].
///
/// Output bytes are byte-for-byte identical to
/// [`ParseResult::binary_bytes`] for the same input.
#[must_use]
pub fn parse_to_bytes(source: &str, options: ParseOptions) -> ParseBytesResult {
    use crate::writer::BinaryWriter;

    let arena = Allocator::default();

    let parser_options = ParseOptions {
        base_offset: 0,
        ..options
    };
    let parsed = context::parse_block_into_data(&arena, source, 0, parser_options);

    let mut writer = BinaryWriter::new(&arena);
    if options.compat_mode {
        writer.set_compat_mode(true);
    }
    let source_offset = writer.append_source_text(source);

    let root_node_index = if parsed.is_failure() {
        0
    } else {
        context::emit_block(&mut writer, &parsed).unwrap_or(0)
    };
    writer.push_root(root_node_index, source_offset, options.base_offset);

    for diag in parsed.diagnostics() {
        writer.push_diagnostic(0, diag.message());
    }

    let diagnostics: Vec<OwnedDiagnostic> = parsed
        .diagnostics()
        .iter()
        .map(|d| OwnedDiagnostic {
            message: d.message(),
            span: d.span,
        })
        .collect();

    let binary_bytes = writer.finish();

    ParseBytesResult {
        binary_bytes,
        diagnostics,
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

/// Arena-backed diagnostic emitted by [`parse_batch`].
///
/// Carries the `root_index` so callers can correlate each diagnostic with
/// the matching `lazy_roots[root_index]` entry without re-decoding the
/// `Diagnostics` section of the Binary AST.
#[derive(Debug, Clone, Copy)]
pub struct BatchDiagnostic<'arena> {
    /// Human-readable description of the issue.
    pub message: &'arena str,
    /// Source span the diagnostic refers to, when known.
    pub span: Option<Span>,
    /// Index of the input item this diagnostic belongs to (`0..items.len()`).
    pub root_index: u32,
}

/// Heap-owned counterpart of [`BatchDiagnostic`] returned by
/// [`parse_batch_to_bytes`].
#[derive(Debug, Clone, Copy)]
pub struct OwnedBatchDiagnostic {
    /// Human-readable description of the issue.
    pub message: &'static str,
    /// Source span the diagnostic refers to, when known.
    pub span: Option<Span>,
    /// Index of the input item this diagnostic belongs to.
    pub root_index: u32,
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
    /// All diagnostics produced during the batch, in input order.
    pub diagnostics: ArenaVec<'arena, BatchDiagnostic<'arena>>,
}

/// Result of [`parse_batch_to_bytes`] — heap-owned bytes + diagnostics.
///
/// Bytes-only sibling of [`BatchResult`]; suitable for handing straight to
/// NAPI/WASM bindings.
#[derive(Debug)]
pub struct ParseBatchBytesResult {
    /// Binary AST bytes — owned, 8-byte aligned by the writer.
    pub binary_bytes: Vec<u8>,
    /// All diagnostics produced during the batch, in input order.
    pub diagnostics: Vec<OwnedBatchDiagnostic>,
}

/// Parse N JSDoc block comments into a single shared Binary AST buffer.
///
/// All N roots share the same `String Data` (so common tag names like
/// `param`, `returns` are interned once), the same Extended Data buffer,
/// and the same Nodes section laid out side-by-side. See
/// `design/007-binary-ast/batch-processing.md` for the format details.
///
/// On parse failure for `items[i]`, `lazy_roots[i]` is `None` and at least
/// one matching diagnostic is recorded with `root_index == i`.
pub fn parse_batch<'arena>(
    arena: &'arena Allocator,
    items: &[BatchItem<'_>],
    options: ParseOptions,
) -> BatchResult<'arena> {
    use crate::writer::BinaryWriter;

    let mut writer = BinaryWriter::new(arena);
    if options.compat_mode {
        writer.set_compat_mode(true);
    }

    // Parse with relative spans (base_offset = 0); each root index entry
    // carries the absolute offset so the lazy decoder can rebuild ranges.
    let parser_options = ParseOptions {
        base_offset: 0,
        ..options
    };

    let mut arena_diagnostics: ArenaVec<'arena, BatchDiagnostic<'arena>> = ArenaVec::new_in(arena);

    for (index, item) in items.iter().enumerate() {
        let root_index = index as u32;
        let source_offset_in_data = writer.append_source_text(item.source_text);

        let parsed = context::parse_block_into_data(arena, item.source_text, 0, parser_options);

        let root_node_index = if parsed.is_failure() {
            0
        } else {
            context::emit_block(&mut writer, &parsed).unwrap_or(0)
        };
        writer.push_root(root_node_index, source_offset_in_data, item.base_offset);

        for diag in parsed.diagnostics() {
            writer.push_diagnostic(root_index, diag.message());
            arena_diagnostics.push(BatchDiagnostic {
                message: arena.alloc_str(diag.message()),
                span: diag.span,
                root_index,
            });
        }
    }

    let bytes_vec = writer.finish();
    let binary_bytes: &'arena [u8] = arena.alloc_slice_copy(&bytes_vec);
    let source_file_owned = LazySourceFile::new(binary_bytes)
        .expect("BinaryWriter::finish() always produces a header-valid buffer");
    let source_file_ref: &'arena LazySourceFile<'arena> = arena.alloc(source_file_owned);

    let mut lazy_roots: ArenaVec<'arena, Option<LazyJsdocBlock<'arena>>> = ArenaVec::new_in(arena);
    for root in source_file_ref.asts() {
        lazy_roots.push(root);
    }

    BatchResult {
        binary_bytes,
        lazy_roots,
        source_file: *source_file_ref,
        diagnostics: arena_diagnostics,
    }
}

/// Bytes-only sibling of [`parse_batch`].
///
/// Skips the arena copy used by [`parse_batch`] for `binary_bytes`; binding
/// code that hands the buffer straight to JS should call this instead.
///
/// Output bytes are byte-for-byte identical to [`BatchResult::binary_bytes`]
/// for the same input.
#[must_use]
pub fn parse_batch_to_bytes(
    items: &[BatchItem<'_>],
    options: ParseOptions,
) -> ParseBatchBytesResult {
    use crate::writer::BinaryWriter;

    let arena = Allocator::default();
    let mut writer = BinaryWriter::new(&arena);
    if options.compat_mode {
        writer.set_compat_mode(true);
    }

    let parser_options = ParseOptions {
        base_offset: 0,
        ..options
    };

    let mut diagnostics: Vec<OwnedBatchDiagnostic> = Vec::new();

    for (index, item) in items.iter().enumerate() {
        let root_index = index as u32;
        let source_offset_in_data = writer.append_source_text(item.source_text);

        let parsed = context::parse_block_into_data(&arena, item.source_text, 0, parser_options);

        let root_node_index = if parsed.is_failure() {
            0
        } else {
            context::emit_block(&mut writer, &parsed).unwrap_or(0)
        };
        writer.push_root(root_node_index, source_offset_in_data, item.base_offset);

        for diag in parsed.diagnostics() {
            writer.push_diagnostic(root_index, diag.message());
            diagnostics.push(OwnedBatchDiagnostic {
                message: diag.message(),
                span: diag.span,
                root_index,
            });
        }
    }

    let binary_bytes = writer.finish();

    ParseBatchBytesResult {
        binary_bytes,
        diagnostics,
    }
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
    fn parse_to_bytes_matches_parse_for_simple_block() {
        let arena = Allocator::default();
        let opts = ParseOptions::default();
        let typed = parse(&arena, "/** ok */", opts);
        let bytes_only = parse_to_bytes("/** ok */", opts);
        assert_eq!(typed.binary_bytes, bytes_only.binary_bytes.as_slice());
        assert_eq!(typed.diagnostics.len(), bytes_only.diagnostics.len());
    }

    #[test]
    fn parse_to_bytes_matches_parse_for_param_tag() {
        let arena = Allocator::default();
        let mut opts = ParseOptions::default();
        opts.parse_types = true;
        let src = "/**\n * @param {string | number} id - The user ID\n */";
        let typed = parse(&arena, src, opts);
        let bytes_only = parse_to_bytes(src, opts);
        assert_eq!(typed.binary_bytes, bytes_only.binary_bytes.as_slice());
    }

    #[test]
    fn parse_to_bytes_matches_parse_with_compat_mode() {
        let arena = Allocator::default();
        let mut opts = ParseOptions::default();
        opts.compat_mode = true;
        let src = "/**\n * Description.\n * @param x The value\n */";
        let typed = parse(&arena, src, opts);
        let bytes_only = parse_to_bytes(src, opts);
        assert_eq!(typed.binary_bytes, bytes_only.binary_bytes.as_slice());
    }

    #[test]
    fn parse_to_bytes_matches_parse_for_failure() {
        let arena = Allocator::default();
        let opts = ParseOptions::default();
        let typed = parse(&arena, "/* plain */", opts);
        let bytes_only = parse_to_bytes("/* plain */", opts);
        assert_eq!(typed.binary_bytes, bytes_only.binary_bytes.as_slice());
        assert_eq!(typed.diagnostics.len(), bytes_only.diagnostics.len());
        assert_eq!(
            typed.diagnostics[0].message,
            bytes_only.diagnostics[0].message
        );
    }

    #[test]
    fn parse_to_bytes_preserves_base_offset() {
        let opts = ParseOptions {
            base_offset: 12345,
            ..ParseOptions::default()
        };
        let arena = Allocator::default();
        let typed = parse(&arena, "/** ok */", opts);
        let bytes_only = parse_to_bytes("/** ok */", opts);
        assert_eq!(typed.binary_bytes, bytes_only.binary_bytes.as_slice());
    }

    #[test]
    fn parse_batch_empty_items_returns_empty_result() {
        let arena = Allocator::default();
        let result = parse_batch(&arena, &[], ParseOptions::default());
        assert_eq!(result.lazy_roots.len(), 0);
        assert_eq!(result.diagnostics.len(), 0);
        assert_eq!(result.source_file.root_count, 0);
    }

    #[test]
    fn parse_batch_single_item_emits_root() {
        let arena = Allocator::default();
        let items = [BatchItem {
            source_text: "/** ok */",
            base_offset: 0,
        }];
        let result = parse_batch(&arena, &items, ParseOptions::default());
        assert_eq!(result.lazy_roots.len(), 1);
        let root = result.lazy_roots[0].expect("root present");
        assert_eq!(root.description(), Some("ok"));
        assert_eq!(result.diagnostics.len(), 0);
    }

    #[test]
    fn parse_batch_multiple_items_each_produces_root() {
        let arena = Allocator::default();
        let items = [
            BatchItem {
                source_text: "/** first */",
                base_offset: 0,
            },
            BatchItem {
                source_text: "/**\n * @param {string} id - second\n */",
                base_offset: 100,
            },
            BatchItem {
                source_text: "/** third */",
                base_offset: 200,
            },
        ];
        let result = parse_batch(&arena, &items, ParseOptions::default());
        assert_eq!(result.lazy_roots.len(), 3);
        assert_eq!(result.lazy_roots[0].unwrap().description(), Some("first"));
        let second_tag = result.lazy_roots[1].unwrap().tags().next().expect("tag");
        assert_eq!(second_tag.tag().value(), "param");
        assert_eq!(result.lazy_roots[2].unwrap().description(), Some("third"));
    }

    #[test]
    fn parse_batch_failure_yields_none_root_and_diagnostic() {
        let arena = Allocator::default();
        let items = [
            BatchItem {
                source_text: "/** good */",
                base_offset: 0,
            },
            BatchItem {
                source_text: "/* not jsdoc */",
                base_offset: 50,
            },
            BatchItem {
                source_text: "/** also good */",
                base_offset: 100,
            },
        ];
        let result = parse_batch(&arena, &items, ParseOptions::default());
        assert_eq!(result.lazy_roots.len(), 3);
        assert!(result.lazy_roots[0].is_some());
        assert!(result.lazy_roots[1].is_none(), "failed parse → None");
        assert!(result.lazy_roots[2].is_some());
        assert!(result.diagnostics.iter().any(|d| d.root_index == 1));
    }

    #[test]
    fn parse_batch_diagnostics_carry_root_index() {
        let arena = Allocator::default();
        let items = [
            BatchItem {
                source_text: "/* first not jsdoc */",
                base_offset: 0,
            },
            BatchItem {
                source_text: "/** ok */",
                base_offset: 100,
            },
            BatchItem {
                source_text: "/* third not jsdoc */",
                base_offset: 200,
            },
        ];
        let result = parse_batch(&arena, &items, ParseOptions::default());
        let indices: Vec<u32> = result.diagnostics.iter().map(|d| d.root_index).collect();
        assert!(indices.contains(&0));
        assert!(indices.contains(&2));
        assert!(!indices.contains(&1));
    }

    #[test]
    fn parse_batch_preserves_base_offset_per_item() {
        let arena = Allocator::default();
        let items = [
            BatchItem {
                source_text: "/** a */",
                base_offset: 1000,
            },
            BatchItem {
                source_text: "/** b */",
                base_offset: 2000,
            },
        ];
        let result = parse_batch(&arena, &items, ParseOptions::default());
        assert_eq!(result.source_file.get_root_base_offset(0), 1000);
        assert_eq!(result.source_file.get_root_base_offset(1), 2000);
    }

    #[test]
    fn parse_batch_to_bytes_matches_parse_batch() {
        let arena = Allocator::default();
        let items = [
            BatchItem {
                source_text: "/** alpha */",
                base_offset: 10,
            },
            BatchItem {
                source_text: "/**\n * @param {string} x - one\n * @returns {number} two\n */",
                base_offset: 200,
            },
            BatchItem {
                source_text: "/* parse failure */",
                base_offset: 500,
            },
        ];
        let opts = ParseOptions::default();
        let typed = parse_batch(&arena, &items, opts);
        let bytes_only = parse_batch_to_bytes(&items, opts);
        assert_eq!(typed.binary_bytes, bytes_only.binary_bytes.as_slice());
        assert_eq!(typed.diagnostics.len(), bytes_only.diagnostics.len());
        for (t, b) in typed.diagnostics.iter().zip(bytes_only.diagnostics.iter()) {
            assert_eq!(t.message, b.message);
            assert_eq!(t.root_index, b.root_index);
        }
    }

    #[test]
    fn parse_batch_dedups_strings_across_roots() {
        // Same comment N times — String Data should not grow linearly with N
        // because the dedup table stores common strings (`*`, `*/`, ` `, the
        // tag name, etc.) once for the whole batch.
        let single_src = "/**\n * @param {string} id\n */";
        let single = parse_to_bytes(single_src, ParseOptions::default());
        let single_size = single.binary_bytes.len();

        let mut items: Vec<BatchItem<'_>> = Vec::with_capacity(50);
        for _ in 0..50 {
            items.push(BatchItem {
                source_text: single_src,
                base_offset: 0,
            });
        }
        let batch = parse_batch_to_bytes(&items, ParseOptions::default());
        // Sanity: 50 roots emitted
        let lazy = LazySourceFile::new(&batch.binary_bytes).unwrap();
        assert_eq!(lazy.root_count, 50);
        // Per-comment cost is much less than a standalone parse (header,
        // string table dedup amortise away).
        let per_comment = batch.binary_bytes.len() / 50;
        assert!(
            per_comment < single_size,
            "per-comment size {per_comment} should be < standalone size {single_size}"
        );
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
