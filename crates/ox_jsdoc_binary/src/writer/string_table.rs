// @author kazuya kawaguchi (a.k.a. kazupon)
// @license MIT
//

//! String table builder used by [`super::BinaryWriter`].
//!
//! Owns the **String Offsets** (`8K` bytes) and **String Data** buffers.
//! Strings are deduplicated by content so that `param`, `returns`, etc.
//! shared across a batch of comments are interned only once.
//!
//! See `design/007-binary-ast/format.md#string-table` for the section
//! layout, and `format::string_table` for the on-wire constants.

use core::num::NonZeroU32;
use std::sync::OnceLock;

use oxc_allocator::{Allocator, Vec as ArenaVec};
use rustc_hash::FxHashMap;

use crate::format::string_table::{STRING_TABLE_MAX_INDEX, U16_NONE_SENTINEL};

/// Index into the **String Offsets** table.
///
/// Newtype wrapper so a string index cannot accidentally be confused with a
/// node index, an extended-data offset, or a raw `u32`. The maximum value
/// fits in a `u16` for Extended Data slots
/// (see `format::string_table::STRING_TABLE_MAX_INDEX`); String-type Node
/// Data leaves can reference up to 30 bits.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct StringIndex(NonZeroU32);

impl StringIndex {
    /// Construct a [`StringIndex`] from a raw `u32`.
    ///
    /// Returns `None` only when `value == u32::MAX` (so `value + 1`
    /// overflows the `NonZeroU32` storage). Index 0 itself is a valid
    /// string index, so the wrapper internally stores `index + 1`.
    #[inline]
    #[must_use]
    pub const fn from_u32(value: u32) -> Option<Self> {
        match NonZeroU32::new(value.wrapping_add(1)) {
            Some(nz) => Some(StringIndex(nz)),
            None => None,
        }
    }

    /// Get the raw `u32` index.
    #[inline]
    #[must_use]
    pub const fn as_u32(self) -> u32 {
        self.0.get() - 1
    }

    /// Get the raw value as `u16`. Returns [`U16_NONE_SENTINEL`] when the
    /// index exceeds [`STRING_TABLE_MAX_INDEX`] — Extended Data slots use
    /// `0xFFFF` as the "None" marker, so that mapping is what the
    /// per-Kind helpers want even when interpretation is ambiguous.
    ///
    /// Realistic encoders never overflow because the writer caps the
    /// string table at [`STRING_TABLE_MAX_INDEX`] entries; this fallback
    /// merely guarantees memory safety if a bug causes overflow.
    #[inline]
    #[must_use]
    pub const fn as_u16(self) -> u16 {
        let raw = self.as_u32();
        if raw <= STRING_TABLE_MAX_INDEX as u32 {
            raw as u16
        } else {
            U16_NONE_SENTINEL
        }
    }
}

/// Builds the String Offsets + String Data buffers incrementally.
///
/// Internally:
/// - the offsets buffer accumulates `(start, end)` u32 pairs (8 bytes each),
/// - the data buffer accumulates UTF-8 bytes (zero-copy slice from the
///   source text whenever possible),
/// - a hash map from `&'arena str` to existing [`StringIndex`] enables dedup.
pub struct StringTableBuilder<'arena> {
    /// `8K` bytes of `(start, end)` u32 pairs, one per interned string.
    pub(crate) offsets_buffer: ArenaVec<'arena, u8>,
    /// Raw concatenated UTF-8 bytes for every interned string and every
    /// appended sourceText.
    pub(crate) data_buffer: ArenaVec<'arena, u8>,
    /// Number of strings interned so far. Equal to
    /// `offsets_buffer.len() / 8`.
    pub(crate) count: u32,
    /// Reference to the underlying arena, used to allocate dedup keys with
    /// `'arena` lifetime.
    arena: &'arena Allocator,
    /// Dedup map: arena-allocated string → its `StringIndex`. Stored on
    /// the `std` heap (not the arena) because `HashMap` cannot live inside
    /// `oxc_allocator` without a custom allocator binding.
    ///
    /// Uses `FxHashMap` (rustc-hash) instead of the default `SipHash` for
    /// ~2x faster hashing on the unique-string slow path. Inputs are
    /// parser-derived AST text (no untrusted user input), so the lack of
    /// HashDoS resistance is acceptable.
    dedup: FxHashMap<&'arena str, StringIndex>,
}

/// Number of common strings pre-interned by [`StringTableBuilder::new`].
/// Useful for tests that assert against the post-construction state.
pub const COMMON_STRING_COUNT: u32 = COMMON_STRINGS.len() as u32;

/// Strings that the writer pre-interns at construction so the common case
/// (delimiters, whitespace, well-known tag names) skips the HashMap entirely.
///
/// Each entry's array index is its predetermined `StringIndex`. The
/// length-bucketed `lookup_common` helper below MUST stay in sync — adding
/// or reordering entries here without updating it will cause the fast path
/// to return wrong indices.
const COMMON_STRINGS: &[&str] = &[
    // 0..=4: source-preserving leaves
    "",
    " ",
    "*",
    "*/",
    "\n",
    // 5..=7: less common whitespace
    "\t",
    "\r\n",
    "/**",
    // 8..=27: most common JSDoc tag names (eslint-plugin-jsdoc usage data)
    "param",
    "returns",
    "return",
    "throws",
    "type",
    "see",
    "example",
    "deprecated",
    "since",
    "default",
    "author",
    "internal",
    "private",
    "public",
    "protected",
    "static",
    "this",
    "override",
    "readonly",
    "yields",
];

/// Length-bucketed perfect-hash style match for [`COMMON_STRINGS`]. Returns
/// the predetermined index when `value` is a common string, otherwise `None`.
///
/// The `match value.len()` arm lets the compiler skip whole comparison
/// chains for non-matching lengths in one branch, which is what makes this
/// path cheaper than the generic `HashMap::get`.
#[inline]
pub(crate) fn lookup_common(value: &str) -> Option<u32> {
    match value.len() {
        0 => Some(0), // ""
        1 => match value.as_bytes()[0] {
            b' ' => Some(1),
            b'*' => Some(2),
            b'\n' => Some(4),
            b'\t' => Some(5),
            _ => None,
        },
        2 => match value {
            "*/" => Some(3),
            "\r\n" => Some(6),
            _ => None,
        },
        3 => match value {
            "/**" => Some(7),
            "see" => Some(13),
            _ => None,
        },
        4 => match value {
            "type" => Some(12),
            "this" => Some(24),
            _ => None,
        },
        5 => match value {
            "param" => Some(8),
            "since" => Some(16),
            _ => None,
        },
        6 => match value {
            "return" => Some(10),
            "throws" => Some(11),
            "author" => Some(18),
            "public" => Some(21),
            "static" => Some(23),
            "yields" => Some(27),
            _ => None,
        },
        7 => match value {
            "returns" => Some(9),
            "example" => Some(14),
            "default" => Some(17),
            "private" => Some(20),
            _ => None,
        },
        8 => match value {
            "internal" => Some(19),
            "readonly" => Some(26),
            "override" => Some(25),
            _ => None,
        },
        9 => match value {
            "protected" => Some(22),
            _ => None,
        },
        10 => match value {
            "deprecated" => Some(15),
            _ => None,
        },
        _ => None,
    }
}

/// Pre-computed `(start, end)` u32-LE pairs for [`COMMON_STRINGS`], cached
/// on first use so every [`StringTableBuilder::new`] is just two memcpys
/// rather than 28 individual HashMap inserts and arena `alloc_str` calls.
fn prelude_offsets() -> &'static [u8] {
    static CACHE: OnceLock<Vec<u8>> = OnceLock::new();
    CACHE.get_or_init(|| {
        let mut buf = Vec::with_capacity(COMMON_STRINGS.len() * 8);
        let mut pos = 0u32;
        for s in COMMON_STRINGS {
            let start = pos;
            pos += s.len() as u32;
            let end = pos;
            buf.extend_from_slice(&start.to_le_bytes());
            buf.extend_from_slice(&end.to_le_bytes());
        }
        buf
    })
}

/// Pre-computed concatenated bytes for [`COMMON_STRINGS`] — same caching
/// rationale as [`prelude_offsets`].
fn prelude_data() -> &'static [u8] {
    static CACHE: OnceLock<Vec<u8>> = OnceLock::new();
    CACHE.get_or_init(|| {
        let mut buf = Vec::new();
        for s in COMMON_STRINGS {
            buf.extend_from_slice(s.as_bytes());
        }
        buf
    })
}

impl<'arena> StringTableBuilder<'arena> {
    /// Create a builder seeded with [`COMMON_STRINGS`] at fixed indices.
    ///
    /// The seeding is two cheap memcpys (offsets + data); the dedup
    /// HashMap stays empty so common strings never enter it — the fast
    /// path in [`Self::intern`] returns early via [`lookup_common`]
    /// before consulting the map.
    #[must_use]
    pub fn new(arena: &'arena Allocator) -> Self {
        let mut offsets_buffer = ArenaVec::new_in(arena);
        offsets_buffer.extend_from_slice(prelude_offsets());

        let mut data_buffer = ArenaVec::new_in(arena);
        data_buffer.extend_from_slice(prelude_data());

        StringTableBuilder {
            offsets_buffer,
            data_buffer,
            count: COMMON_STRING_COUNT,
            arena,
            dedup: FxHashMap::default(),
        }
    }

    /// Intern `value` and return its [`StringIndex`].
    ///
    /// Fast path: [`lookup_common`] returns the predetermined index for
    /// the well-known strings pre-seeded by [`Self::new`], avoiding the
    /// HashMap lookup entirely.
    ///
    /// Slow path: regular HashMap dedup, falling back to a fresh entry.
    pub fn intern(&mut self, value: &str) -> StringIndex {
        if let Some(idx) = lookup_common(value) {
            // SAFETY: `idx` < COMMON_STRINGS.len() < u32::MAX, so
            // `from_u32` always succeeds.
            return StringIndex::from_u32(idx).expect("common index in range");
        }
        if let Some(&existing) = self.dedup.get(value) {
            return existing;
        }
        self.intern_uncached(value)
    }

    /// Internal: append a fresh entry without consulting the fast path or
    /// the dedup map. Used by [`Self::new`] to seed the common strings
    /// (the fast path is not yet usable until they have been written) and
    /// by [`Self::intern`] for the genuine cache-miss case.
    fn intern_uncached(&mut self, value: &str) -> StringIndex {
        let start = self.data_buffer.len() as u32;
        self.data_buffer.extend_from_slice(value.as_bytes());
        let end = self.data_buffer.len() as u32;
        self.offsets_buffer.extend_from_slice(&start.to_le_bytes());
        self.offsets_buffer.extend_from_slice(&end.to_le_bytes());

        let idx = StringIndex::from_u32(self.count).expect("string index overflow");
        self.count = self.count.checked_add(1).expect("string table overflow");

        // Allocate a stable &'arena str so the dedup key outlives every
        // future call.
        let key: &'arena str = self.arena.alloc_str(value);
        self.dedup.insert(key, idx);
        idx
    }

    /// Append `value` as a fresh entry with neither the [`lookup_common`]
    /// fast path nor the dedup HashMap consulted. Use this for strings
    /// the caller knows are dominated by unique-per-call values
    /// (description-line text, raw `{type}` source, etc.) where paying
    /// the FxHash + lookup work and the arena `alloc_str` for a key that
    /// will never be revisited is pure overhead.
    ///
    /// Trade-off: identical content called twice produces two distinct
    /// `StringIndex` values (so two offsets entries + two data copies).
    /// The decoder reads either one as the same string, so correctness
    /// is unaffected; only the binary grows by ~8 bytes per duplicate.
    pub fn intern_unique(&mut self, value: &str) -> StringIndex {
        let start = self.data_buffer.len() as u32;
        self.data_buffer.extend_from_slice(value.as_bytes());
        let end = self.data_buffer.len() as u32;
        self.offsets_buffer.extend_from_slice(&start.to_le_bytes());
        self.offsets_buffer.extend_from_slice(&end.to_le_bytes());

        let idx = StringIndex::from_u32(self.count).expect("string index overflow");
        self.count = self.count.checked_add(1).expect("string table overflow");
        idx
    }

    /// Append a String Offsets entry that points into an existing range of
    /// `data_buffer` — typically the source text region appended via
    /// [`Self::append_source_text`] — **without copying any bytes**.
    ///
    /// This is the zero-copy intern path used when the caller knows the
    /// string content already lives somewhere in the data buffer (e.g. it
    /// is a slice of the just-appended source text). It collapses the
    /// per-call cost from "FxHash probe + `data_buffer` write +
    /// `offsets_buffer` write" down to just the `offsets_buffer` write,
    /// which is the dominant emit-phase saving identified in
    /// `.notes/binary-ast-emit-phase-format-analysis.md` (Path A).
    ///
    /// Caller's contract: `[start, end)` MUST be a valid byte range that
    /// already lives inside `data_buffer`. Passing an out-of-range range
    /// produces a corrupted binary (decoder will read junk bytes); the
    /// caller is responsible for the bounds.
    pub fn intern_at_offset(&mut self, start: u32, end: u32) -> StringIndex {
        debug_assert!(
            (end as usize) <= self.data_buffer.len(),
            "intern_at_offset range [{start}, {end}) extends past data_buffer length {}",
            self.data_buffer.len()
        );
        debug_assert!(start <= end, "intern_at_offset start > end");
        self.offsets_buffer.extend_from_slice(&start.to_le_bytes());
        self.offsets_buffer.extend_from_slice(&end.to_le_bytes());

        let idx = StringIndex::from_u32(self.count).expect("string index overflow");
        self.count = self.count.checked_add(1).expect("string table overflow");
        idx
    }

    /// Append a sourceText prefix without registering it in the offsets
    /// table.
    ///
    /// Used by the writer to register each `BatchItem.sourceText` at the
    /// front of String Data so that `Pos`/`End` slicing is direct. Returns
    /// the byte offset where the text was written (used to populate
    /// `root[i].source_offset_in_data`).
    ///
    /// Caller's contract: must be invoked **before** any [`Self::intern`]
    /// call so source-text bytes occupy the front of the String Data
    /// section, matching the format spec ("[sourceText[0]] [sourceText[1]]
    /// ... [unique strings...]").
    pub fn append_source_text(&mut self, value: &str) -> u32 {
        let offset = self.data_buffer.len() as u32;
        self.data_buffer.extend_from_slice(value.as_bytes());
        offset
    }

    /// Number of strings interned so far.
    #[inline]
    #[must_use]
    pub const fn len(&self) -> u32 {
        self.count
    }

    /// Whether nothing has been interned yet.
    #[inline]
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.count == 0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn string_index_round_trips() {
        for raw in [0u32, 1, 100, 0xFFFE, 0x3FFF_FFFE] {
            let idx = StringIndex::from_u32(raw).unwrap();
            assert_eq!(idx.as_u32(), raw);
        }
    }

    #[test]
    fn string_index_rejects_overflow() {
        assert!(StringIndex::from_u32(u32::MAX).is_none());
    }

    #[test]
    fn string_index_zero_is_representable() {
        let idx = StringIndex::from_u32(0).unwrap();
        assert_eq!(idx.as_u32(), 0);
    }

    #[test]
    fn intern_dedups_repeated_values() {
        let arena = Allocator::default();
        let mut builder = StringTableBuilder::new(&arena);
        // Use names that are NOT in COMMON_STRINGS so we exercise the
        // HashMap dedup path rather than the fast-path lookup.
        let a = builder.intern("custom_alpha");
        let b = builder.intern("custom_beta");
        let a_again = builder.intern("custom_alpha");
        assert_eq!(a, a_again, "intern must dedup identical strings");
        assert_ne!(a, b);
        assert_eq!(
            builder.len(),
            COMMON_STRING_COUNT + 2,
            "two unique entries beyond the common prelude"
        );
    }

    #[test]
    fn intern_appends_to_offsets_and_data_buffers() {
        let arena = Allocator::default();
        let mut builder = StringTableBuilder::new(&arena);
        let common_offsets_bytes = builder.offsets_buffer.len();
        let common_data_bytes = builder.data_buffer.len();
        let _ = builder.intern("ab");
        let _ = builder.intern("cd");
        assert_eq!(
            builder.offsets_buffer.len() - common_offsets_bytes,
            16,
            "two new 8-byte entries beyond the common prelude"
        );
        assert_eq!(
            builder.data_buffer.len() - common_data_bytes,
            4,
            "two new strings appended (2 bytes each)"
        );
    }

    #[test]
    fn append_source_text_does_not_register_an_index() {
        let arena = Allocator::default();
        let mut builder = StringTableBuilder::new(&arena);
        let common_data_bytes = builder.data_buffer.len() as u32;
        let off = builder.append_source_text("/** @x */");
        assert_eq!(
            off, common_data_bytes,
            "source text starts immediately after the common-string prelude"
        );
        assert_eq!(
            builder.len(),
            COMMON_STRING_COUNT,
            "source text does not count as an interned entry"
        );
        assert_eq!(
            builder.data_buffer.len(),
            common_data_bytes as usize + "/** @x */".len()
        );
    }

    #[test]
    fn intern_common_string_returns_predetermined_index() {
        let arena = Allocator::default();
        let mut builder = StringTableBuilder::new(&arena);
        // "param" is COMMON_STRINGS[8].
        assert_eq!(builder.intern("param").as_u32(), 8);
        // Repeating the same string returns the same index without
        // bumping the count.
        let pre_count = builder.len();
        assert_eq!(builder.intern("param").as_u32(), 8);
        assert_eq!(builder.len(), pre_count, "fast path must not append");
    }
}
