//! Lazy structs for the 15 comment AST kinds (`0x01 - 0x0F`).
//!
//! Phase 1.0c: every struct is a `Copy` value type wrapping
//! `(source_file, node_index)`. Method bodies are `todo!()` placeholders;
//! Phase 1.1b fills them in by reading the underlying byte slice.

use crate::format::kind::Kind;

use super::super::source_file::LazySourceFile;
use super::{LazyNode, NodeListIter};

/// Generate a lazy comment AST struct + its `LazyNode` impl in one go.
///
/// The macro keeps every wrapper at exactly `&'a LazySourceFile + u32` —
/// 16 bytes on 64-bit targets — so that traversal stays heap-free.
macro_rules! define_lazy_comment_node {
    ($name:ident, $kind:expr, $doc:expr) => {
        #[doc = $doc]
        #[derive(Debug, Clone, Copy)]
        pub struct $name<'a> {
            source_file: &'a LazySourceFile<'a>,
            node_index: u32,
        }

        impl<'a> LazyNode<'a> for $name<'a> {
            const KIND: Kind = $kind;

            #[inline]
            fn from_index(source_file: &'a LazySourceFile<'a>, node_index: u32) -> Self {
                $name { source_file, node_index }
            }

            #[inline]
            fn source_file(&self) -> &'a LazySourceFile<'a> {
                self.source_file
            }

            #[inline]
            fn node_index(&self) -> u32 {
                self.node_index
            }
        }
    };
}

// ---------------------------------------------------------------------------
// 0x01 LazyJsdocBlock — root of one `/** ... */` block
// ---------------------------------------------------------------------------
define_lazy_comment_node!(
    LazyJsdocBlock,
    Kind::JsdocBlock,
    "Lazy view of a `JsdocBlock` (Kind 0x01, root node)."
);

impl<'a> LazyJsdocBlock<'a> {
    /// `[absolute_pos, absolute_end]` (UTF-16 code units, original-file basis).
    pub fn range(&self) -> [u32; 2] {
        todo!("Phase 1.1b: read Pos/End + add base_offset for the owning root")
    }

    /// `description` string (or `None` when absent).
    pub fn description(&self) -> Option<&'a str> {
        todo!("Phase 1.1b: read u16 string index from Extended Data byte 2-3")
    }

    /// Source-preserving delimiter strings written into Extended Data.
    pub fn delimiter(&self) -> &'a str { todo!("Phase 1.1b: Extended Data byte 4-5") }
    /// `post_delimiter` source string.
    pub fn post_delimiter(&self) -> &'a str { todo!("Phase 1.1b: Extended Data byte 6-7") }
    /// `terminal` source string.
    pub fn terminal(&self) -> &'a str { todo!("Phase 1.1b: Extended Data byte 8-9") }
    /// `line_end` source string.
    pub fn line_end(&self) -> &'a str { todo!("Phase 1.1b: Extended Data byte 10-11") }
    /// `initial` source string (indentation before `/**`).
    pub fn initial(&self) -> &'a str { todo!("Phase 1.1b: Extended Data byte 12-13") }
    /// `delimiter_line_break` source string (`"\n"` for multi-line, `""` otherwise).
    pub fn delimiter_line_break(&self) -> &'a str { todo!("Phase 1.1b: Extended Data byte 14-15") }
    /// `preterminal_line_break` source string.
    pub fn preterminal_line_break(&self) -> &'a str { todo!("Phase 1.1b: Extended Data byte 16-17") }

    /// Top-level description lines as a NodeList iterator (visitor bit 0).
    pub fn description_lines(&self) -> NodeListIter<'a, LazyJsdocDescriptionLine<'a>> {
        todo!("Phase 1.1b: visitor index 0 in JsdocBlock Children bitmask")
    }
    /// Block tags as a NodeList iterator (visitor bit 1).
    pub fn tags(&self) -> NodeListIter<'a, LazyJsdocTag<'a>> {
        todo!("Phase 1.1b: visitor index 1")
    }
    /// Inline tags found in the top-level description (visitor bit 2).
    pub fn inline_tags(&self) -> NodeListIter<'a, LazyJsdocInlineTag<'a>> {
        todo!("Phase 1.1b: visitor index 2")
    }

    // -- compat-mode-only line metadata (Extended Data byte 20+) -------------

    /// 0-based line index of the closing `*/` line. Returns `None` when the
    /// buffer was not written in compat mode.
    pub fn end_line(&self) -> Option<u32> { todo!("Phase 1.1b: compat tail byte 20-23") }
    /// First line of the block description.
    pub fn description_start_line(&self) -> Option<u32> { todo!("Phase 1.1b: compat tail byte 24-27") }
    /// Last line of the block description.
    pub fn description_end_line(&self) -> Option<u32> { todo!("Phase 1.1b: compat tail byte 28-31") }
    /// Last description line that still belongs to the description block.
    pub fn last_description_line(&self) -> Option<u32> { todo!("Phase 1.1b: compat tail byte 32-35") }
    /// `1` when the block description text exists on the closing `*/` line.
    pub fn has_preterminal_description(&self) -> Option<u8> { todo!("Phase 1.1b: compat tail byte 36") }
    /// `Some(1)` when the last tag description is on the closing `*/` line.
    pub fn has_preterminal_tag_description(&self) -> Option<u8> { todo!("Phase 1.1b: compat tail byte 37") }
}

// ---------------------------------------------------------------------------
// 0x02 LazyJsdocDescriptionLine
// ---------------------------------------------------------------------------
define_lazy_comment_node!(
    LazyJsdocDescriptionLine,
    Kind::JsdocDescriptionLine,
    "Lazy view of a `JsdocDescriptionLine` (Kind 0x02)."
);

impl<'a> LazyJsdocDescriptionLine<'a> {
    /// Description content after stripping the JSDoc margin.
    pub fn description(&self) -> &'a str { todo!("Phase 1.1b: String-type Node Data payload (basic) or Extended (compat)") }
    /// `delimiter` source string. Only emitted in compat mode; `None` otherwise.
    pub fn delimiter(&self) -> Option<&'a str> { todo!("Phase 1.1b: Extended Data byte 2-3 in compat") }
    /// `post_delimiter` source string. Only emitted in compat mode.
    pub fn post_delimiter(&self) -> Option<&'a str> { todo!("Phase 1.1b: Extended Data byte 4-5 in compat") }
    /// `initial` source string. Only emitted in compat mode.
    pub fn initial(&self) -> Option<&'a str> { todo!("Phase 1.1b: Extended Data byte 6-7 in compat") }
}

// ---------------------------------------------------------------------------
// 0x03 LazyJsdocTag
// ---------------------------------------------------------------------------
define_lazy_comment_node!(
    LazyJsdocTag,
    Kind::JsdocTag,
    "Lazy view of a `JsdocTag` (Kind 0x03)."
);

impl<'a> LazyJsdocTag<'a> {
    /// `[absolute_pos, absolute_end]` for this tag.
    pub fn range(&self) -> [u32; 2] { todo!("Phase 1.1b: Pos/End + base_offset") }

    /// Whether the tag was written with bracket syntax such as `[id]`.
    pub fn optional(&self) -> bool { todo!("Phase 1.1b: Common Data bit0") }

    // -- direct-string fields (Extended Data) --------------------------------
    /// `default_value` string from `[id=foo]` syntax.
    pub fn default_value(&self) -> Option<&'a str> { todo!("Phase 1.1b: Extended Data byte 2-3") }
    /// Joined description text after the type/name.
    pub fn description(&self) -> Option<&'a str> { todo!("Phase 1.1b: Extended Data byte 4-5") }
    /// Raw body when the tag uses the `Raw` body variant.
    pub fn raw_body(&self) -> Option<&'a str> { todo!("Phase 1.1b: Extended Data byte 6-7") }

    // -- child nodes (Children bitmask) -------------------------------------
    /// Tag name child (`@name`). Always present (bit0 of the bitmask is required).
    pub fn tag(&self) -> LazyJsdocTagName<'a> { todo!("Phase 1.1b: visitor index 0") }
    /// Raw `{...}` type source. `None` when the tag has no `{...}` part.
    pub fn raw_type(&self) -> Option<LazyJsdocTypeSource<'a>> { todo!("Phase 1.1b: visitor index 1") }
    /// First-name child for tags like `@param NAME`.
    pub fn name(&self) -> Option<LazyJsdocTagNameValue<'a>> { todo!("Phase 1.1b: visitor index 2") }
    /// Parsed type AST, when `parsedType` is enabled.
    ///
    /// Returns `None` whenever bit3 of the Children bitmask is unset; the
    /// caller need not check `compat_mode`. Phase 1.1b will resolve the
    /// child node and return the appropriate variant of [`super::type_node::LazyTypeNode`].
    pub fn parsed_type(&self) -> Option<super::type_node::LazyTypeNode<'a>> {
        todo!("Phase 1.1b: visitor index 3, dispatch on child Kind")
    }
    /// Body wrapper covering `Generic` / `Borrows` / `Raw` body variants.
    pub fn body(&self) -> Option<LazyJsdocTagBody<'a>> { todo!("Phase 1.1b: visitor index 4") }

    // -- NodeList children ---------------------------------------------------
    /// Source-preserving type-source lines.
    pub fn type_lines(&self) -> NodeListIter<'a, LazyJsdocTypeLine<'a>> { todo!("Phase 1.1b: visitor index 5") }
    /// Source-preserving description lines.
    pub fn description_lines(&self) -> NodeListIter<'a, LazyJsdocDescriptionLine<'a>> {
        todo!("Phase 1.1b: visitor index 6")
    }
    /// Inline tags found in this tag's description.
    pub fn inline_tags(&self) -> NodeListIter<'a, LazyJsdocInlineTag<'a>> { todo!("Phase 1.1b: visitor index 7") }
}

// ---------------------------------------------------------------------------
// 0x04 LazyJsdocTagName (String leaf)
// ---------------------------------------------------------------------------
define_lazy_comment_node!(
    LazyJsdocTagName,
    Kind::JsdocTagName,
    "Lazy view of a `JsdocTagName` leaf (Kind 0x04)."
);

impl<'a> LazyJsdocTagName<'a> {
    /// Tag name without the leading `@` (e.g. `"param"`).
    pub fn value(&self) -> &'a str { todo!("Phase 1.1b: read 30-bit String payload, resolve via String table") }
}

// ---------------------------------------------------------------------------
// 0x05 LazyJsdocTagNameValue (String leaf)
// ---------------------------------------------------------------------------
define_lazy_comment_node!(
    LazyJsdocTagNameValue,
    Kind::JsdocTagNameValue,
    "Lazy view of a `JsdocTagNameValue` leaf (Kind 0x05)."
);

impl<'a> LazyJsdocTagNameValue<'a> {
    /// Raw name token (e.g. `id` from `@param id`).
    pub fn raw(&self) -> &'a str { todo!("Phase 1.1b: 30-bit String payload") }
}

// ---------------------------------------------------------------------------
// 0x06 LazyJsdocTypeSource (String leaf)
// ---------------------------------------------------------------------------
define_lazy_comment_node!(
    LazyJsdocTypeSource,
    Kind::JsdocTypeSource,
    "Lazy view of a `JsdocTypeSource` leaf (Kind 0x06; raw `{...}` text)."
);

impl<'a> LazyJsdocTypeSource<'a> {
    /// Raw text inside `{...}` (without the braces).
    pub fn raw(&self) -> &'a str { todo!("Phase 1.1b: 30-bit String payload") }
}

// ---------------------------------------------------------------------------
// 0x07 LazyJsdocTypeLine (String basic / Extended compat)
// ---------------------------------------------------------------------------
define_lazy_comment_node!(
    LazyJsdocTypeLine,
    Kind::JsdocTypeLine,
    "Lazy view of a `JsdocTypeLine` (Kind 0x07)."
);

impl<'a> LazyJsdocTypeLine<'a> {
    /// Raw `{...}` line content.
    pub fn raw_type(&self) -> &'a str { todo!("Phase 1.1b: String payload (basic) or Extended Data byte 0-1 (compat)") }
    /// `delimiter` source string. Only emitted in compat mode.
    pub fn delimiter(&self) -> Option<&'a str> { todo!("Phase 1.1b: Extended Data byte 2-3 in compat") }
    /// `post_delimiter` source string. Only emitted in compat mode.
    pub fn post_delimiter(&self) -> Option<&'a str> { todo!("Phase 1.1b: Extended Data byte 4-5 in compat") }
    /// `initial` source string. Only emitted in compat mode.
    pub fn initial(&self) -> Option<&'a str> { todo!("Phase 1.1b: Extended Data byte 6-7 in compat") }
}

// ---------------------------------------------------------------------------
// 0x08 LazyJsdocInlineTag
// ---------------------------------------------------------------------------
define_lazy_comment_node!(
    LazyJsdocInlineTag,
    Kind::JsdocInlineTag,
    "Lazy view of a `JsdocInlineTag` (Kind 0x08)."
);

impl<'a> LazyJsdocInlineTag<'a> {
    /// Inline tag format (Plain / Pipe / Space / Prefix / Unknown). Stored in
    /// Common Data bits[0:2].
    pub fn format(&self) -> u8 { todo!("Phase 1.1b: Common Data bits[0:2]") }
    /// Tag name child (`@link` etc.).
    pub fn tag(&self) -> LazyJsdocTagName<'a> { todo!("Phase 1.1b: child node") }
    /// Optional name path or URL portion.
    pub fn namepath_or_url(&self) -> Option<&'a str> { todo!("Phase 1.1b: Extended Data byte 0-1") }
    /// Optional display text portion.
    pub fn text(&self) -> Option<&'a str> { todo!("Phase 1.1b: Extended Data byte 2-3") }
    /// Raw body text fallback.
    pub fn raw_body(&self) -> Option<&'a str> { todo!("Phase 1.1b: Extended Data byte 4-5") }
}

// ---------------------------------------------------------------------------
// 0x09 LazyJsdocGenericTagBody
// ---------------------------------------------------------------------------
define_lazy_comment_node!(
    LazyJsdocGenericTagBody,
    Kind::JsdocGenericTagBody,
    "Lazy view of a `JsdocGenericTagBody` (Kind 0x09)."
);

impl<'a> LazyJsdocGenericTagBody<'a> {
    /// `true` when the tag separator was `-`. Stored in Common Data bit0.
    pub fn has_dash_separator(&self) -> bool { todo!("Phase 1.1b: Common Data bit0") }
    /// `description` string after the dash separator.
    pub fn description(&self) -> Option<&'a str> { todo!("Phase 1.1b: Extended Data byte 2-3") }
    /// Type source child (`{T}`).
    pub fn type_source(&self) -> Option<LazyJsdocTypeSource<'a>> { todo!("Phase 1.1b: visitor index 0") }
    /// Tag value child (parameter name / namepath / identifier / raw text).
    pub fn value(&self) -> Option<LazyJsdocTagValue<'a>> { todo!("Phase 1.1b: visitor index 1") }
}

// ---------------------------------------------------------------------------
// 0x0A LazyJsdocBorrowsTagBody
// ---------------------------------------------------------------------------
define_lazy_comment_node!(
    LazyJsdocBorrowsTagBody,
    Kind::JsdocBorrowsTagBody,
    "Lazy view of a `JsdocBorrowsTagBody` (Kind 0x0A)."
);

impl<'a> LazyJsdocBorrowsTagBody<'a> {
    /// Source name (the side being borrowed from).
    pub fn source(&self) -> LazyJsdocNamepathSource<'a> { todo!("Phase 1.1b: visitor index 0") }
    /// Target name (the side receiving the borrow).
    pub fn target(&self) -> LazyJsdocNamepathSource<'a> { todo!("Phase 1.1b: visitor index 1") }
}

// ---------------------------------------------------------------------------
// 0x0B LazyJsdocRawTagBody (String leaf)
// ---------------------------------------------------------------------------
define_lazy_comment_node!(
    LazyJsdocRawTagBody,
    Kind::JsdocRawTagBody,
    "Lazy view of a `JsdocRawTagBody` leaf (Kind 0x0B)."
);

impl<'a> LazyJsdocRawTagBody<'a> {
    /// Raw body text.
    pub fn raw(&self) -> &'a str { todo!("Phase 1.1b: 30-bit String payload") }
}

// ---------------------------------------------------------------------------
// 0x0C LazyJsdocParameterName
// ---------------------------------------------------------------------------
define_lazy_comment_node!(
    LazyJsdocParameterName,
    Kind::JsdocParameterName,
    "Lazy view of a `JsdocParameterName` (Kind 0x0C)."
);

impl<'a> LazyJsdocParameterName<'a> {
    /// `true` when the parameter was wrapped in brackets (`[id]`).
    pub fn optional(&self) -> bool { todo!("Phase 1.1b: Common Data bit0") }
    /// The path text (e.g. `id` or `options.timeout`).
    pub fn path(&self) -> &'a str { todo!("Phase 1.1b: Extended Data byte 0-1") }
    /// Default value from `[id=foo]` syntax.
    pub fn default_value(&self) -> Option<&'a str> { todo!("Phase 1.1b: Extended Data byte 2-3") }
}

// ---------------------------------------------------------------------------
// 0x0D LazyJsdocNamepathSource (String leaf)
// ---------------------------------------------------------------------------
define_lazy_comment_node!(
    LazyJsdocNamepathSource,
    Kind::JsdocNamepathSource,
    "Lazy view of a `JsdocNamepathSource` leaf (Kind 0x0D)."
);

impl<'a> LazyJsdocNamepathSource<'a> {
    /// Raw namepath text.
    pub fn raw(&self) -> &'a str { todo!("Phase 1.1b: 30-bit String payload") }
}

// ---------------------------------------------------------------------------
// 0x0E LazyJsdocIdentifier (String leaf)
// ---------------------------------------------------------------------------
define_lazy_comment_node!(
    LazyJsdocIdentifier,
    Kind::JsdocIdentifier,
    "Lazy view of a `JsdocIdentifier` leaf (Kind 0x0E)."
);

impl<'a> LazyJsdocIdentifier<'a> {
    /// Identifier text.
    pub fn name(&self) -> &'a str { todo!("Phase 1.1b: 30-bit String payload") }
}

// ---------------------------------------------------------------------------
// 0x0F LazyJsdocText (String leaf, `JsdocTagValue::Raw` variant)
// ---------------------------------------------------------------------------
define_lazy_comment_node!(
    LazyJsdocText,
    Kind::JsdocText,
    "Lazy view of a `JsdocText` leaf (Kind 0x0F)."
);

impl<'a> LazyJsdocText<'a> {
    /// Raw text value.
    pub fn value(&self) -> &'a str { todo!("Phase 1.1b: 30-bit String payload") }
}

// ---------------------------------------------------------------------------
// Variant wrappers
// ---------------------------------------------------------------------------

/// Body of a `JsdocTag` — one of three variants distinguished by the child Kind.
///
/// Returned by [`LazyJsdocTag::body`]. The wrapping type is needed because
/// the on-wire format uses three distinct Kinds (per the design choice of
/// expanding Rust enum variants into independent Kinds — see
/// `design/007-binary-ast/encoding.md`).
#[derive(Debug, Clone, Copy)]
pub enum LazyJsdocTagBody<'a> {
    /// Generic body (`@param {T} name - desc`).
    Generic(LazyJsdocGenericTagBody<'a>),
    /// Borrows body (`@borrows from as to`).
    Borrows(LazyJsdocBorrowsTagBody<'a>),
    /// Raw text body (anything that did not match Generic/Borrows shapes).
    Raw(LazyJsdocRawTagBody<'a>),
}

/// Tag value — first non-type token after the tag name.
///
/// Returned by [`LazyJsdocGenericTagBody::value`]. As with [`LazyJsdocTagBody`],
/// each Rust variant is its own Binary AST Kind.
#[derive(Debug, Clone, Copy)]
pub enum LazyJsdocTagValue<'a> {
    /// Parameter-style name (e.g. `[id=foo]`).
    Parameter(LazyJsdocParameterName<'a>),
    /// Namepath token.
    Namepath(LazyJsdocNamepathSource<'a>),
    /// Bare identifier.
    Identifier(LazyJsdocIdentifier<'a>),
    /// Raw text fallback.
    Raw(LazyJsdocText<'a>),
}

#[cfg(test)]
mod tests {
    use super::*;
    use core::mem::size_of;

    /// Lazy nodes must fit in 16 bytes (`&LazySourceFile` 8 + `u32` 4 +
    /// alignment padding 4 = 16) to keep `Box::new`-free traversal fast.
    #[test]
    fn comment_ast_lazy_structs_fit_in_16_bytes() {
        macro_rules! assert_size {
            ($t:ty) => {
                assert!(
                    size_of::<$t>() <= 16,
                    concat!(stringify!($t), " exceeds 16 bytes; lazy nodes must stay register-friendly")
                );
            };
        }
        assert_size!(LazyJsdocBlock<'static>);
        assert_size!(LazyJsdocDescriptionLine<'static>);
        assert_size!(LazyJsdocTag<'static>);
        assert_size!(LazyJsdocTagName<'static>);
        assert_size!(LazyJsdocTagNameValue<'static>);
        assert_size!(LazyJsdocTypeSource<'static>);
        assert_size!(LazyJsdocTypeLine<'static>);
        assert_size!(LazyJsdocInlineTag<'static>);
        assert_size!(LazyJsdocGenericTagBody<'static>);
        assert_size!(LazyJsdocBorrowsTagBody<'static>);
        assert_size!(LazyJsdocRawTagBody<'static>);
        assert_size!(LazyJsdocParameterName<'static>);
        assert_size!(LazyJsdocNamepathSource<'static>);
        assert_size!(LazyJsdocIdentifier<'static>);
        assert_size!(LazyJsdocText<'static>);
    }

    /// Variant wrappers are allowed to be a few bytes larger because of the
    /// discriminant, but they must still fit comfortably on the stack.
    #[test]
    fn variant_wrappers_fit_in_24_bytes() {
        assert!(size_of::<LazyJsdocTagBody<'static>>() <= 24);
        assert!(size_of::<LazyJsdocTagValue<'static>>() <= 24);
    }
}
