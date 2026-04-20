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

    /// Construct a lazy node from `(source_file, node_index)`.
    ///
    /// Implementations must `debug_assert!` that the Kind byte at
    /// `nodes_offset + node_index * 24` equals `Self::KIND`.
    fn from_index(source_file: &'a LazySourceFile<'a>, node_index: u32) -> Self;

    /// Borrow the [`LazySourceFile`] this node came from.
    fn source_file(&self) -> &'a LazySourceFile<'a>;

    /// The index of this node within the Nodes section.
    fn node_index(&self) -> u32;
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
    _marker: PhantomData<T>,
}

impl<'a, T: LazyNode<'a>> NodeListIter<'a, T> {
    /// Create a new iterator that starts at `head_index`. `head_index = 0`
    /// produces an immediately-empty iterator.
    #[inline]
    #[must_use]
    pub const fn new(source_file: &'a LazySourceFile<'a>, head_index: u32) -> Self {
        NodeListIter {
            source_file,
            current_index: head_index,
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
        // Phase 1.1b: construct T::from_index(...) and advance to the
        // current node's `next_sibling`. Today we just terminate to avoid
        // calling `todo!()` from a public iterator API.
        let _ = (&self.source_file, self.current_index);
        self.current_index = 0;
        None
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
        // ptr (8) + u32 (4) + PhantomData (0) + alignment padding (4) = 16
        assert!(size_of::<NodeListIter<'static, ()>>() <= 16);
    }
}
