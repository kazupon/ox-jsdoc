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
//! Phase 1.0d: only the public surface (signatures, types) is in place.
//! [`parse`] panics with `unimplemented!()`, and [`parse_batch`] is
//! reserved for Phase 2 (per `design/007-binary-ast/phases.md`
//! "Phase 2: Batch support + public encoder API").

use oxc_allocator::{Allocator, Vec as ArenaVec};
use oxc_span::Span;

use crate::decoder::nodes::comment_ast::LazyJsdocBlock;
use crate::decoder::source_file::LazySourceFile;

/// Options controlling parser behaviour.
///
/// Most ox-jsdoc users will leave every field at its [`Default`] value; the
/// fields exist so binding code can flip [`compat_mode`] for jsdoccomment
/// compatibility (see `design/007-binary-ast/encoding.md`).
///
/// [`compat_mode`]: ParseOptions::compat_mode
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
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
/// Phase 1.2a will deliver the actual implementation; Phase 1.0d ships the
/// signature so downstream skeleton crates (NAPI/WASM bindings) can compile.
pub fn parse<'arena>(
    _arena: &'arena Allocator,
    _source: &'arena str,
    _options: ParseOptions,
) -> ParseResult<'arena> {
    unimplemented!(
        "Phase 1.2a: invoke the parser-integrated writer for a single comment, then \
         construct LazySourceFile from the finished bytes"
    )
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
}
