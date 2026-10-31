// @author kazuya kawaguchi (a.k.a. kazupon)
// @license MIT
//

//! `LazySourceFile::new` rejects a buffer whose String Data is not UTF-8 or
//! whose sections do not fit in it, instead of giving strings that are not
//! UTF-8 or panicking later.

use ox_jsdoc::decoder::error::DecodeError;
use ox_jsdoc::decoder::source_file::LazySourceFile;
use ox_jsdoc::format::header::{
    DIAGNOSTICS_OFFSET_FIELD, EXTENDED_DATA_OFFSET_FIELD, NODE_COUNT_FIELD, NODES_OFFSET_FIELD,
    ROOT_ARRAY_OFFSET_FIELD, ROOT_COUNT_FIELD, STRING_DATA_OFFSET_FIELD,
    STRING_OFFSETS_OFFSET_FIELD,
};
use ox_jsdoc::parser::{ParseOptions, parse};
use oxc_allocator::Allocator;

const SOURCE: &str = "/** é and ü\n * @param {string} a the a\n */";

/// The bytes of `SOURCE` and the offset of its source text in them.
fn buffer() -> (Vec<u8>, usize) {
    let arena = Allocator::default();
    let result = parse(&arena, SOURCE, ParseOptions::default());
    let source_file = result.source_file;
    let source = source_file.string_data_offset as usize
        + source_file.get_root_source_offset_in_data(0) as usize;
    (result.binary_bytes.to_vec(), source)
}

/// `bytes` with the u32 Header field at `field` set to `value`.
fn with_field(bytes: &[u8], field: usize, value: u32) -> Vec<u8> {
    let mut bytes = bytes.to_vec();
    bytes[field..field + 4].copy_from_slice(&value.to_le_bytes());
    bytes
}

fn read_field(bytes: &[u8], field: usize) -> u32 {
    u32::from_le_bytes(bytes[field..field + 4].try_into().expect("4 bytes"))
}

#[test]
fn accepts_the_buffers_of_the_writer() {
    let (bytes, _) = buffer();
    let source_file = LazySourceFile::new(&bytes).expect("valid buffer");
    let block = source_file.asts().next().flatten().expect("parsed");
    assert_eq!(block.description(), Some("é and ü"));
    // SAFETY: the writer of this crate wrote the buffer.
    let unchecked = unsafe { LazySourceFile::new_unchecked(&bytes) }.expect("valid buffer");
    let block = unchecked.asts().next().flatten().expect("parsed");
    assert_eq!(block.description(), Some("é and ü"));
}

#[test]
fn rejects_string_data_that_is_not_utf8() {
    let (mut bytes, source) = buffer();
    assert_eq!(bytes[source + 4], 0xC3, "the first byte of `é`");
    bytes[source + 4] = 0xFF;
    assert!(
        matches!(
            LazySourceFile::new(&bytes),
            Err(DecodeError::InvalidStringData { .. })
        ),
        "{:?}",
        LazySourceFile::new(&bytes)
    );
}

#[test]
fn rejects_sections_that_do_not_fit() {
    let (bytes, _) = buffer();
    let len = bytes.len() as u32;
    let string_data = read_field(&bytes, STRING_DATA_OFFSET_FIELD);
    let nodes = read_field(&bytes, NODES_OFFSET_FIELD);
    for (what, broken) in [
        (
            "Root Index Array in the Header",
            with_field(&bytes, ROOT_ARRAY_OFFSET_FIELD, 8),
        ),
        (
            "String Offsets before the Root Index Array",
            with_field(&bytes, STRING_OFFSETS_OFFSET_FIELD, 40),
        ),
        (
            "String Data past the end",
            with_field(&bytes, STRING_DATA_OFFSET_FIELD, len + 100),
        ),
        (
            "Extended Data before String Data",
            with_field(&bytes, EXTENDED_DATA_OFFSET_FIELD, string_data - 4),
        ),
        (
            "Diagnostics past the nodes",
            with_field(&bytes, DIAGNOSTICS_OFFSET_FIELD, nodes + 4),
        ),
        (
            "nodes past the end",
            with_field(&bytes, NODES_OFFSET_FIELD, len + 24),
        ),
        (
            "more nodes than fit",
            with_field(&bytes, NODE_COUNT_FIELD, 1_000_000),
        ),
        (
            "more roots than fit",
            with_field(&bytes, ROOT_COUNT_FIELD, 1_000_000),
        ),
    ] {
        assert!(
            matches!(
                LazySourceFile::new(&broken),
                Err(DecodeError::InvalidSectionLayout { .. })
            ),
            "{what}: {:?}",
            LazySourceFile::new(&broken)
        );
        // The unchecked constructor skips only the UTF-8 check.
        // SAFETY: the String Data is still the valid one of the writer.
        assert!(
            matches!(
                unsafe { LazySourceFile::new_unchecked(&broken) },
                Err(DecodeError::InvalidSectionLayout { .. })
            ),
            "{what} (unchecked)"
        );
    }
}
