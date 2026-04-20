//! Lazy node structs (60 in total) and the [`LazyNode`] trait.
//!
//! Per `design/007-binary-ast/rust-impl.md#lazy-nodes-are-stack-value-types-no-box-allocation`,
//! every lazy node is a `#[derive(Copy, Clone)]` struct of at most ~32 bytes
//! that holds a slice into the buffer plus its `node_index`. Eliminating
//! the per-traversal `Box::new` is the main reason the Rust walker is
//! ~10x faster than the typed-AST baseline in earlier ox-jsdoc benchmarks.

pub mod comment_ast;
pub mod type_node;

use core::marker::PhantomData;

use crate::format::kind::Kind;
use crate::format::node_record::{
    COMMON_DATA_MASK, END_OFFSET, KIND_OFFSET, NODE_RECORD_SIZE, PARENT_INDEX_OFFSET, POS_OFFSET,
};

use super::helpers::read_u32;
use super::source_file::LazySourceFile;

/// Common interface implemented by every lazy node struct.
///
/// `KIND` enables compile-time `from_index` validation (`debug_assert!` on
/// the byte at `nodes_offset + node_index * 24`). The accessor methods give
/// generic helpers (e.g. [`NodeListIter`]) a uniform way to construct child
/// instances.
pub trait LazyNode<'a>: Copy + Clone + Sized {
    /// The Kind value this struct represents.
    const KIND: Kind;

    /// Construct a lazy node from `(source_file, node_index, root_index)`.
    ///
    /// `root_index` is the index of the [`crate::decoder::source_file::LazySourceFile::asts`]
    /// entry that contains this node, propagated from parent to child so
    /// that [`Self::range`] can compute absolute positions in O(1).
    ///
    /// Implementations must `debug_assert!` that the Kind byte at
    /// `nodes_offset + node_index * 24` equals `Self::KIND`.
    fn from_index(
        source_file: &'a LazySourceFile<'a>,
        node_index: u32,
        root_index: u32,
    ) -> Self;

    /// Borrow the [`LazySourceFile`] this node came from.
    fn source_file(&self) -> &'a LazySourceFile<'a>;

    /// The index of this node within the Nodes section.
    fn node_index(&self) -> u32;

    /// The root this node belongs to.
    fn root_index(&self) -> u32;

    // ----- default-method getters shared by every Lazy* struct -----

    /// Byte offset of this node's record within the Nodes section.
    #[inline]
    fn byte_offset(&self) -> usize {
        self.source_file().nodes_offset as usize
            + self.node_index() as usize * NODE_RECORD_SIZE
    }

    /// Read the `Kind` byte and decode it via [`Kind::from_u8`].
    #[inline]
    fn kind(&self) -> Kind {
        let byte = self.source_file().bytes()[self.byte_offset() + KIND_OFFSET];
        Kind::from_u8(byte).expect("encoder must only emit defined Kinds")
    }

    /// Read the 6-bit Common Data byte (upper 2 bits masked off).
    #[inline]
    fn common_data(&self) -> u8 {
        self.source_file().bytes()[self.byte_offset() + 1] & COMMON_DATA_MASK
    }

    /// `Pos` field — UTF-16 code unit offset *relative to the root's
    /// sourceText*.
    #[inline]
    fn pos(&self) -> u32 {
        read_u32(self.source_file().bytes(), self.byte_offset() + POS_OFFSET)
    }

    /// `End` field (relative to the root's sourceText).
    #[inline]
    fn end(&self) -> u32 {
        read_u32(self.source_file().bytes(), self.byte_offset() + END_OFFSET)
    }

    /// `[absolute_pos, absolute_end]` — `Pos`/`End` plus the root's
    /// `base_offset`. Use this when feeding ranges into ESLint reports.
    #[inline]
    fn range(&self) -> [u32; 2] {
        let base = self.source_file().get_root_base_offset(self.root_index());
        [base + self.pos(), base + self.end()]
    }

    /// Index of this node's parent. `0` means the parent is the
    /// [`crate::format::root_index::PARSE_FAILURE_SENTINEL`] (i.e. this
    /// node is a root).
    #[inline]
    fn parent_index(&self) -> u32 {
        read_u32(
            self.source_file().bytes(),
            self.byte_offset() + PARENT_INDEX_OFFSET,
        )
    }
}

/// Iterator yielded by lazy "NodeList" getters.
///
/// Stored as a tiny value-type struct so that calling `.tags()` on a parent
/// allocates nothing — the iterator itself sits on the stack and walks
/// `next_sibling` links on each `next()` call.
#[derive(Debug, Clone, Copy)]
pub struct NodeListIter<'a, T> {
    source_file: &'a LazySourceFile<'a>,
    /// Current position in the Nodes section. `0` means "end of list"
    /// because the sentinel `node[0]` is reused as the no-link marker.
    current_index: u32,
    /// Root index propagated to every yielded child.
    root_index: u32,
    _marker: PhantomData<T>,
}

impl<'a, T: LazyNode<'a>> NodeListIter<'a, T> {
    /// Create a new iterator that starts at `head_index`. `head_index = 0`
    /// produces an immediately-empty iterator.
    #[inline]
    #[must_use]
    pub const fn new(
        source_file: &'a LazySourceFile<'a>,
        head_index: u32,
        root_index: u32,
    ) -> Self {
        NodeListIter {
            source_file,
            current_index: head_index,
            root_index,
            _marker: PhantomData,
        }
    }

    /// Whether the iterator has been fully consumed.
    #[inline]
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.current_index == 0
    }
}

impl<'a, T: LazyNode<'a>> Iterator for NodeListIter<'a, T> {
    type Item = T;

    fn next(&mut self) -> Option<Self::Item> {
        if self.current_index == 0 {
            return None;
        }
        let item = T::from_index(self.source_file, self.current_index, self.root_index);
        self.current_index = super::helpers::read_next_sibling(self.source_file, self.current_index);
        Some(item)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use core::mem::size_of;

    /// `NodeListIter` is one of the values that crosses every traversal
    /// boundary, so it must stay tiny enough to pass through registers on
    /// every supported target.
    #[test]
    fn node_list_iter_is_compact() {
        // ptr (8) + u32 (4) + u32 (4) + PhantomData (0) = 16
        assert!(size_of::<NodeListIter<'static, ()>>() <= 24);
    }
}
