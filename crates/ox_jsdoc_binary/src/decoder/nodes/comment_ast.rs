// @author kazuya kawaguchi (a.k.a. kazupon)
// @license MIT
//

//! Lazy structs for the 15 comment AST kinds (`0x01 - 0x0F`).
//!
//! Every struct is a `Copy` value type wrapping
//! `(source_file, node_index, root_index)` (16 bytes on 64-bit targets).
//! Phase 1.1b ships full `LazyNode` impls + the per-Kind getters that the
//! Phase 1.2a parser and Phase 1.1c visitor will rely on.

use crate::format::kind::Kind;
use crate::format::node_record::{KIND_OFFSET, NODE_RECORD_SIZE};
use crate::format::string_table::U16_NONE_SENTINEL;

use super::super::helpers::{
    child_at_visitor_index, ext_offset, first_child, read_u16, string_payload,
};
use super::super::source_file::LazySourceFile;
use super::type_node::LazyTypeNode;
use super::{LazyNode, NodeListIter};

/// Generate a lazy comment AST struct + its `LazyNode` impl in one go.
macro_rules! define_lazy_comment_node {
    ($name:ident, $kind:expr, $doc:expr) => {
        #[doc = $doc]
        #[derive(Debug, Clone, Copy)]
        pub struct $name<'a> {
            source_file: &'a LazySourceFile<'a>,
            node_index: u32,
            root_index: u32,
        }

        impl<'a> LazyNode<'a> for $name<'a> {
            const KIND: Kind = $kind;

            #[inline]
            fn from_index(
                source_file: &'a LazySourceFile<'a>,
                node_index: u32,
                root_index: u32,
            ) -> Self {
                $name {
                    source_file,
                    node_index,
                    root_index,
                }
            }

            #[inline]
            fn source_file(&self) -> &'a LazySourceFile<'a> {
                self.source_file
            }

            #[inline]
            fn node_index(&self) -> u32 {
                self.node_index
            }

            #[inline]
            fn root_index(&self) -> u32 {
                self.root_index
            }
        }
    };
}

/// Resolve a u16 string slot in Extended Data into an `Option<&str>`.
#[inline]
fn resolve_u16_slot<'a>(sf: &LazySourceFile<'a>, ext_byte: usize, field_offset: usize) -> Option<&'a str> {
    let idx = read_u16(sf.bytes(), ext_byte + field_offset);
    if idx == U16_NONE_SENTINEL {
        None
    } else {
        sf.get_string(idx as u32)
    }
}

// ---------------------------------------------------------------------------
// 0x01 LazyJsdocBlock
// ---------------------------------------------------------------------------
define_lazy_comment_node!(
    LazyJsdocBlock,
    Kind::JsdocBlock,
    "Lazy view of a `JsdocBlock` (Kind 0x01, root node)."
);

impl<'a> LazyJsdocBlock<'a> {
    /// Children bitmask (`bit0=descLines`, `bit1=tags`, `bit2=inlineTags`).
    #[inline]
    fn children_bitmask(&self) -> u8 {
        let ext = ext_offset(self.source_file, self.node_index) as usize;
        self.source_file.bytes()[ext]
    }

    /// Description string (None when absent).
    pub fn description(&self) -> Option<&'a str> {
        let ext = ext_offset(self.source_file, self.node_index) as usize;
        resolve_u16_slot(self.source_file, ext, 2)
    }

    /// Source-preserving delimiter strings stored in Extended Data.
    pub fn delimiter(&self) -> &'a str {
        ext_string(self.source_file, self.node_index, 4)
    }
    /// `post_delimiter`.
    pub fn post_delimiter(&self) -> &'a str {
        ext_string(self.source_file, self.node_index, 6)
    }
    /// `terminal` source string (`"*/"`).
    pub fn terminal(&self) -> &'a str {
        ext_string(self.source_file, self.node_index, 8)
    }
    /// `line_end` source string.
    pub fn line_end(&self) -> &'a str {
        ext_string(self.source_file, self.node_index, 10)
    }
    /// `initial` source string.
    pub fn initial(&self) -> &'a str {
        ext_string(self.source_file, self.node_index, 12)
    }
    /// `delimiter_line_break` source string.
    pub fn delimiter_line_break(&self) -> &'a str {
        ext_string(self.source_file, self.node_index, 14)
    }
    /// `preterminal_line_break` source string.
    pub fn preterminal_line_break(&self) -> &'a str {
        ext_string(self.source_file, self.node_index, 16)
    }

    /// Top-level description lines (visitor index 0).
    pub fn description_lines(&self) -> NodeListIter<'a, LazyJsdocDescriptionLine<'a>> {
        self.children_node_list(0)
    }
    /// Block tags (visitor index 1).
    pub fn tags(&self) -> NodeListIter<'a, LazyJsdocTag<'a>> {
        self.children_node_list(1)
    }
    /// Inline tags found in the top-level description (visitor index 2).
    pub fn inline_tags(&self) -> NodeListIter<'a, LazyJsdocInlineTag<'a>> {
        self.children_node_list(2)
    }

    // -- compat-mode-only line metadata (Extended Data byte 20+) -------------

    /// 0-based line index of the closing `*/` line. Returns `None` when the
    /// buffer was not written in compat mode.
    pub fn end_line(&self) -> Option<u32> {
        if !self.source_file.compat_mode {
            return None;
        }
        let ext = ext_offset(self.source_file, self.node_index) as usize;
        Some(super::super::helpers::read_u32(self.source_file.bytes(), ext + 20))
    }

    /// Helper: build a NodeListIter for visitor index `i` of this block.
    fn children_node_list<T: LazyNode<'a>>(&self, visitor_index: u8) -> NodeListIter<'a, T> {
        let bitmask = self.children_bitmask();
        if let Some(node_list_index) =
            child_at_visitor_index(self.source_file, self.node_index, bitmask, visitor_index)
        {
            // The slot is always wrapped in a NodeList (Kind 0x7F); the
            // children of that NodeList are the actual elements.
            let head = first_child(self.source_file, node_list_index).unwrap_or(0);
            NodeListIter::new(self.source_file, head, self.root_index)
        } else {
            NodeListIter::new(self.source_file, 0, self.root_index)
        }
    }
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
    /// Description content. When `compat_mode` is off, it lives directly in
    /// Node Data as a String payload; when on, it lives at byte 0-1 of
    /// Extended Data.
    pub fn description(&self) -> &'a str {
        if self.source_file.compat_mode {
            ext_string(self.source_file, self.node_index, 0)
        } else {
            string_payload(self.source_file, self.node_index).unwrap_or("")
        }
    }
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
    /// Whether the tag was written with bracket syntax such as `[id]`.
    #[inline]
    pub fn optional(&self) -> bool {
        (self.common_data() & 0b0000_0001) != 0
    }

    /// `default_value` from `[id=foo]` syntax.
    pub fn default_value(&self) -> Option<&'a str> {
        let ext = ext_offset(self.source_file, self.node_index) as usize;
        resolve_u16_slot(self.source_file, ext, 2)
    }
    /// Joined description text.
    pub fn description(&self) -> Option<&'a str> {
        let ext = ext_offset(self.source_file, self.node_index) as usize;
        resolve_u16_slot(self.source_file, ext, 4)
    }
    /// Raw body when the tag uses the `Raw` body variant.
    pub fn raw_body(&self) -> Option<&'a str> {
        let ext = ext_offset(self.source_file, self.node_index) as usize;
        resolve_u16_slot(self.source_file, ext, 6)
    }

    #[inline]
    fn children_bitmask(&self) -> u8 {
        let ext = ext_offset(self.source_file, self.node_index) as usize;
        self.source_file.bytes()[ext]
    }

    /// Tag name child (`@name`). Always present.
    pub fn tag(&self) -> LazyJsdocTagName<'a> {
        let bitmask = self.children_bitmask();
        let idx = child_at_visitor_index(self.source_file, self.node_index, bitmask, 0)
            .expect("JsdocTag.tag is required");
        LazyJsdocTagName::from_index(self.source_file, idx, self.root_index)
    }
    /// Raw `{...}` type source.
    pub fn raw_type(&self) -> Option<LazyJsdocTypeSource<'a>> {
        let bitmask = self.children_bitmask();
        child_at_visitor_index(self.source_file, self.node_index, bitmask, 1).map(|idx| {
            LazyJsdocTypeSource::from_index(self.source_file, idx, self.root_index)
        })
    }
    /// Tag-name value (e.g. `id` in `@param id`).
    pub fn name(&self) -> Option<LazyJsdocTagNameValue<'a>> {
        let bitmask = self.children_bitmask();
        child_at_visitor_index(self.source_file, self.node_index, bitmask, 2).map(|idx| {
            LazyJsdocTagNameValue::from_index(self.source_file, idx, self.root_index)
        })
    }
    /// `parsedType` child (any TypeNode variant).
    pub fn parsed_type(&self) -> Option<LazyTypeNode<'a>> {
        let bitmask = self.children_bitmask();
        let idx = child_at_visitor_index(self.source_file, self.node_index, bitmask, 3)?;
        LazyTypeNode::from_index(self.source_file, idx, self.root_index)
    }
    /// `body` child (one of `JsdocGenericTagBody` / `JsdocBorrowsTagBody` /
    /// `JsdocRawTagBody`). The variant is determined from the child's Kind
    /// byte; returns `None` when bit4 of the Children bitmask is unset or
    /// the child is not one of the three body kinds.
    pub fn body(&self) -> Option<LazyJsdocTagBody<'a>> {
        let bitmask = self.children_bitmask();
        let idx = child_at_visitor_index(self.source_file, self.node_index, bitmask, 4)?;
        let kind_byte = self.source_file.bytes()
            [self.source_file.nodes_offset as usize + idx as usize * NODE_RECORD_SIZE + KIND_OFFSET];
        let kind = Kind::from_u8(kind_byte).ok()?;
        match kind {
            Kind::JsdocGenericTagBody => Some(LazyJsdocTagBody::Generic(
                LazyJsdocGenericTagBody::from_index(self.source_file, idx, self.root_index),
            )),
            Kind::JsdocBorrowsTagBody => Some(LazyJsdocTagBody::Borrows(
                LazyJsdocBorrowsTagBody::from_index(self.source_file, idx, self.root_index),
            )),
            Kind::JsdocRawTagBody => Some(LazyJsdocTagBody::Raw(
                LazyJsdocRawTagBody::from_index(self.source_file, idx, self.root_index),
            )),
            _ => None,
        }
    }

    /// Source-preserving description lines (visitor index 6, NodeList).
    pub fn description_lines(&self) -> NodeListIter<'a, LazyJsdocDescriptionLine<'a>> {
        self.children_node_list(6)
    }
    /// Source-preserving type lines (visitor index 5, NodeList).
    pub fn type_lines(&self) -> NodeListIter<'a, LazyJsdocTypeLine<'a>> {
        self.children_node_list(5)
    }
    /// Inline tags found in this tag's description (visitor index 7, NodeList).
    pub fn inline_tags(&self) -> NodeListIter<'a, LazyJsdocInlineTag<'a>> {
        self.children_node_list(7)
    }

    fn children_node_list<T: LazyNode<'a>>(&self, visitor_index: u8) -> NodeListIter<'a, T> {
        let bitmask = self.children_bitmask();
        if let Some(node_list_index) =
            child_at_visitor_index(self.source_file, self.node_index, bitmask, visitor_index)
        {
            let head = first_child(self.source_file, node_list_index).unwrap_or(0);
            NodeListIter::new(self.source_file, head, self.root_index)
        } else {
            NodeListIter::new(self.source_file, 0, self.root_index)
        }
    }
}

// ---------------------------------------------------------------------------
// 0x04-0x06, 0x0B, 0x0D-0x0F: String-type leaves
// ---------------------------------------------------------------------------

macro_rules! define_string_leaf {
    ($name:ident, $kind:expr, $accessor:ident, $doc:expr) => {
        define_lazy_comment_node!($name, $kind, $doc);
        impl<'a> $name<'a> {
            #[doc = concat!("Resolve the underlying string value of this `", stringify!($name), "`.")]
            pub fn $accessor(&self) -> &'a str {
                string_payload(self.source_file, self.node_index).unwrap_or("")
            }
        }
    };
}

define_string_leaf!(
    LazyJsdocTagName,
    Kind::JsdocTagName,
    value,
    "Lazy view of a `JsdocTagName` leaf (Kind 0x04)."
);
define_string_leaf!(
    LazyJsdocTagNameValue,
    Kind::JsdocTagNameValue,
    raw,
    "Lazy view of a `JsdocTagNameValue` leaf (Kind 0x05)."
);
define_string_leaf!(
    LazyJsdocTypeSource,
    Kind::JsdocTypeSource,
    raw,
    "Lazy view of a `JsdocTypeSource` leaf (Kind 0x06)."
);
define_string_leaf!(
    LazyJsdocRawTagBody,
    Kind::JsdocRawTagBody,
    raw,
    "Lazy view of a `JsdocRawTagBody` leaf (Kind 0x0B)."
);
define_string_leaf!(
    LazyJsdocNamepathSource,
    Kind::JsdocNamepathSource,
    raw,
    "Lazy view of a `JsdocNamepathSource` leaf (Kind 0x0D)."
);
define_string_leaf!(
    LazyJsdocIdentifier,
    Kind::JsdocIdentifier,
    name,
    "Lazy view of a `JsdocIdentifier` leaf (Kind 0x0E)."
);
define_string_leaf!(
    LazyJsdocText,
    Kind::JsdocText,
    value,
    "Lazy view of a `JsdocText` leaf (Kind 0x0F)."
);

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
    pub fn raw_type(&self) -> &'a str {
        if self.source_file.compat_mode {
            ext_string(self.source_file, self.node_index, 0)
        } else {
            string_payload(self.source_file, self.node_index).unwrap_or("")
        }
    }
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
    /// Inline tag format (`bits[0:2]` of Common Data).
    #[inline]
    pub fn format(&self) -> u8 {
        self.common_data() & 0b0000_0111
    }
    /// Optional name path or URL portion.
    pub fn namepath_or_url(&self) -> Option<&'a str> {
        let ext = ext_offset(self.source_file, self.node_index) as usize;
        resolve_u16_slot(self.source_file, ext, 0)
    }
    /// Optional display text portion.
    pub fn text(&self) -> Option<&'a str> {
        let ext = ext_offset(self.source_file, self.node_index) as usize;
        resolve_u16_slot(self.source_file, ext, 2)
    }
    /// Raw body text fallback.
    pub fn raw_body(&self) -> Option<&'a str> {
        let ext = ext_offset(self.source_file, self.node_index) as usize;
        resolve_u16_slot(self.source_file, ext, 4)
    }
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
    /// `true` when the tag separator was `-`.
    #[inline]
    pub fn has_dash_separator(&self) -> bool {
        (self.common_data() & 0b0000_0001) != 0
    }
    /// `description` string after the dash separator.
    pub fn description(&self) -> Option<&'a str> {
        let ext = ext_offset(self.source_file, self.node_index) as usize;
        resolve_u16_slot(self.source_file, ext, 2)
    }
}

// ---------------------------------------------------------------------------
// 0x0A LazyJsdocBorrowsTagBody
// ---------------------------------------------------------------------------
define_lazy_comment_node!(
    LazyJsdocBorrowsTagBody,
    Kind::JsdocBorrowsTagBody,
    "Lazy view of a `JsdocBorrowsTagBody` (Kind 0x0A)."
);

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
    #[inline]
    pub fn optional(&self) -> bool {
        (self.common_data() & 0b0000_0001) != 0
    }
    /// The path text.
    pub fn path(&self) -> &'a str {
        ext_string(self.source_file, self.node_index, 0)
    }
    /// Default value from `[id=foo]` syntax.
    pub fn default_value(&self) -> Option<&'a str> {
        let ext = ext_offset(self.source_file, self.node_index) as usize;
        resolve_u16_slot(self.source_file, ext, 2)
    }
}

// ---------------------------------------------------------------------------
// helpers
// ---------------------------------------------------------------------------

/// Read a required string field at `ext_byte_offset` inside the node's
/// Extended Data record.
#[inline]
fn ext_string<'a>(sf: &LazySourceFile<'a>, node_index: u32, field_offset: usize) -> &'a str {
    let ext = ext_offset(sf, node_index) as usize;
    let idx = read_u16(sf.bytes(), ext + field_offset);
    sf.get_string(idx as u32).unwrap_or("")
}

// ---------------------------------------------------------------------------
// Variant wrappers
// ---------------------------------------------------------------------------

/// Body of a `JsdocTag` — one of three variants distinguished by the child Kind.
#[derive(Debug, Clone, Copy)]
pub enum LazyJsdocTagBody<'a> {
    /// Generic body (`@param {T} name - desc`).
    Generic(LazyJsdocGenericTagBody<'a>),
    /// Borrows body (`@borrows from as to`).
    Borrows(LazyJsdocBorrowsTagBody<'a>),
    /// Raw text body.
    Raw(LazyJsdocRawTagBody<'a>),
}

/// Tag value — first non-type token after the tag name.
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

    #[test]
    fn lazy_comment_structs_are_compact() {
        macro_rules! assert_size {
            ($t:ty) => {
                assert!(
                    size_of::<$t>() <= 16,
                    concat!(stringify!($t), " exceeds 16 bytes")
                );
            };
        }
        assert_size!(LazyJsdocBlock<'static>);
        assert_size!(LazyJsdocDescriptionLine<'static>);
        assert_size!(LazyJsdocTag<'static>);
        assert_size!(LazyJsdocTagName<'static>);
        assert_size!(LazyJsdocText<'static>);
        assert_size!(LazyJsdocInlineTag<'static>);
        assert_size!(LazyJsdocParameterName<'static>);
    }

    #[test]
    fn variant_wrappers_fit_in_24_bytes() {
        assert!(size_of::<LazyJsdocTagBody<'static>>() <= 24);
        assert!(size_of::<LazyJsdocTagValue<'static>>() <= 24);
    }
}
