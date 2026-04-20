//! `write_*` helpers for the 45 TypeNode kinds (`0x80 - 0xAC`).
//!
//! Phase 1.0b: signatures only; bodies are `unimplemented!()`.
//!
//! TypeNodes split into 4 patterns (see `format.md` "Node catalog matrix"):
//!
//! - **Pattern 1 — String only** (5 kinds): payload is a 30-bit string index.
//! - **Pattern 2 — Children only** (29 kinds): payload is a 30-bit Children
//!   bitmask.
//! - **Pattern 3 — Mixed string + children** (6 kinds): payload is a
//!   30-bit Extended Data offset.
//! - **Others** (5 kinds): pure leaves with no payload (`TypeNull` / `TypeUndefined`
//!   / `TypeAny` / `TypeUnknown` / `TypeUniqueSymbol`).

use oxc_span::Span;

use super::super::{BinaryWriter, StringIndex};
use super::NodeIndex;

// ===========================================================================
// Pattern 1: String only (5 kinds, payload = string index)
// ===========================================================================

/// `TypeName` (Kind `0x80`, Pattern 1).
pub fn write_type_name(
    _writer: &mut BinaryWriter<'_>,
    _span: Span,
    _parent_index: NodeIndex,
    _value: StringIndex,
) -> NodeIndex {
    unimplemented!("Phase 1.1a: emit Kind 0x80 + 30-bit string payload")
}

/// `TypeNumber` (Kind `0x81`, Pattern 1).
pub fn write_type_number(
    _writer: &mut BinaryWriter<'_>,
    _span: Span,
    _parent_index: NodeIndex,
    _value: StringIndex,
) -> NodeIndex {
    unimplemented!("Phase 1.1a: emit Kind 0x81")
}

/// `TypeStringValue` (Kind `0x82`, Pattern 1).
///
/// Common Data: `bits[0:1] = quote` (3-state).
pub fn write_type_string_value(
    _writer: &mut BinaryWriter<'_>,
    _span: Span,
    _parent_index: NodeIndex,
    _quote: u8,
    _value: StringIndex,
) -> NodeIndex {
    unimplemented!("Phase 1.1a: emit Kind 0x82")
}

/// `TypeProperty` (Kind `0xA3`, Pattern 1).
///
/// Common Data: `bits[0:1] = quote` (3-state).
pub fn write_type_property(
    _writer: &mut BinaryWriter<'_>,
    _span: Span,
    _parent_index: NodeIndex,
    _quote: u8,
    _value: StringIndex,
) -> NodeIndex {
    unimplemented!("Phase 1.1a: emit Kind 0xA3")
}

/// `TypeSpecialNamePath` (Kind `0x8F`, Pattern 1).
///
/// Common Data: `bits[0:1] = special_type` (3 variants) + `bits[2:3] = quote`.
pub fn write_type_special_name_path(
    _writer: &mut BinaryWriter<'_>,
    _span: Span,
    _parent_index: NodeIndex,
    _special_type: u8,
    _quote: u8,
    _value: StringIndex,
) -> NodeIndex {
    unimplemented!("Phase 1.1a: emit Kind 0x8F")
}

// ===========================================================================
// Pattern 2: Children only (29 kinds, payload = Children bitmask)
// ===========================================================================

/// `TypeUnion` (Kind `0x87`, Pattern 2; child = NodeList).
pub fn write_type_union(
    _writer: &mut BinaryWriter<'_>,
    _span: Span,
    _parent_index: NodeIndex,
    _children_bitmask: u32,
) -> NodeIndex {
    unimplemented!("Phase 1.1a: emit Kind 0x87")
}

/// `TypeIntersection` (Kind `0x88`, Pattern 2; child = NodeList).
pub fn write_type_intersection(
    _writer: &mut BinaryWriter<'_>,
    _span: Span,
    _parent_index: NodeIndex,
    _children_bitmask: u32,
) -> NodeIndex {
    unimplemented!("Phase 1.1a: emit Kind 0x88")
}

/// `TypeGeneric` (Kind `0x89`, Pattern 2).
///
/// Common Data: `bit0 = brackets`, `bit1 = dot`. Children: 1 + NodeList.
pub fn write_type_generic(
    _writer: &mut BinaryWriter<'_>,
    _span: Span,
    _parent_index: NodeIndex,
    _brackets: u8,
    _dot: bool,
    _children_bitmask: u32,
) -> NodeIndex {
    unimplemented!("Phase 1.1a: emit Kind 0x89")
}

/// `TypeFunction` (Kind `0x8A`, Pattern 2).
///
/// Common Data: `bit0=constructor`, `bit1=arrow`, `bit2=parenthesis`.
pub fn write_type_function(
    _writer: &mut BinaryWriter<'_>,
    _span: Span,
    _parent_index: NodeIndex,
    _constructor: bool,
    _arrow: bool,
    _parenthesis: bool,
    _children_bitmask: u32,
) -> NodeIndex {
    unimplemented!("Phase 1.1a: emit Kind 0x8A")
}

/// `TypeObject` (Kind `0x8B`, Pattern 2).
///
/// Common Data: `bits[0:2] = separator`. Child = NodeList.
pub fn write_type_object(
    _writer: &mut BinaryWriter<'_>,
    _span: Span,
    _parent_index: NodeIndex,
    _separator: u8,
    _children_bitmask: u32,
) -> NodeIndex {
    unimplemented!("Phase 1.1a: emit Kind 0x8B")
}

/// `TypeTuple` (Kind `0x8C`, Pattern 2; child = NodeList).
pub fn write_type_tuple(
    _writer: &mut BinaryWriter<'_>,
    _span: Span,
    _parent_index: NodeIndex,
    _children_bitmask: u32,
) -> NodeIndex {
    unimplemented!("Phase 1.1a: emit Kind 0x8C")
}

/// `TypeParenthesis` (Kind `0x8D`, Pattern 2; 1 child).
pub fn write_type_parenthesis(
    _writer: &mut BinaryWriter<'_>,
    _span: Span,
    _parent_index: NodeIndex,
    _children_bitmask: u32,
) -> NodeIndex {
    unimplemented!("Phase 1.1a: emit Kind 0x8D")
}

/// `TypeNamePath` (Kind `0x8E`, Pattern 2).
///
/// Common Data: `bits[0:1] = path_type`. Children: 2 (left + right).
pub fn write_type_name_path(
    _writer: &mut BinaryWriter<'_>,
    _span: Span,
    _parent_index: NodeIndex,
    _path_type: u8,
    _children_bitmask: u32,
) -> NodeIndex {
    unimplemented!("Phase 1.1a: emit Kind 0x8E")
}

/// `TypeNullable` (Kind `0x90`, Pattern 2; 1 child).
///
/// Common Data: `bit0 = position` (Prefix/Suffix).
pub fn write_type_nullable(
    _writer: &mut BinaryWriter<'_>,
    _span: Span,
    _parent_index: NodeIndex,
    _position: u8,
    _children_bitmask: u32,
) -> NodeIndex {
    unimplemented!("Phase 1.1a: emit Kind 0x90")
}

/// `TypeNotNullable` (Kind `0x91`, Pattern 2; 1 child).
///
/// Common Data: `bit0 = position`.
pub fn write_type_not_nullable(
    _writer: &mut BinaryWriter<'_>,
    _span: Span,
    _parent_index: NodeIndex,
    _position: u8,
    _children_bitmask: u32,
) -> NodeIndex {
    unimplemented!("Phase 1.1a: emit Kind 0x91")
}

/// `TypeOptional` (Kind `0x92`, Pattern 2; 1 child).
///
/// Common Data: `bit0 = position`.
pub fn write_type_optional(
    _writer: &mut BinaryWriter<'_>,
    _span: Span,
    _parent_index: NodeIndex,
    _position: u8,
    _children_bitmask: u32,
) -> NodeIndex {
    unimplemented!("Phase 1.1a: emit Kind 0x92")
}

/// `TypeVariadic` (Kind `0x93`, Pattern 2; 1 child).
///
/// Common Data: `bit0 = position`, `bit1 = square_brackets`.
pub fn write_type_variadic(
    _writer: &mut BinaryWriter<'_>,
    _span: Span,
    _parent_index: NodeIndex,
    _position: u8,
    _square_brackets: bool,
    _children_bitmask: u32,
) -> NodeIndex {
    unimplemented!("Phase 1.1a: emit Kind 0x93")
}

/// `TypeConditional` (Kind `0x94`, Pattern 2; 4 children).
pub fn write_type_conditional(
    _writer: &mut BinaryWriter<'_>,
    _span: Span,
    _parent_index: NodeIndex,
    _children_bitmask: u32,
) -> NodeIndex {
    unimplemented!("Phase 1.1a: emit Kind 0x94")
}

/// `TypeInfer` (Kind `0x95`, Pattern 2; 1 child).
pub fn write_type_infer(
    _writer: &mut BinaryWriter<'_>,
    _span: Span,
    _parent_index: NodeIndex,
    _children_bitmask: u32,
) -> NodeIndex {
    unimplemented!("Phase 1.1a: emit Kind 0x95")
}

/// `TypeKeyOf` (Kind `0x96`, Pattern 2; 1 child).
pub fn write_type_key_of(
    _writer: &mut BinaryWriter<'_>,
    _span: Span,
    _parent_index: NodeIndex,
    _children_bitmask: u32,
) -> NodeIndex {
    unimplemented!("Phase 1.1a: emit Kind 0x96")
}

/// `TypeTypeOf` (Kind `0x97`, Pattern 2; 1 child).
pub fn write_type_type_of(
    _writer: &mut BinaryWriter<'_>,
    _span: Span,
    _parent_index: NodeIndex,
    _children_bitmask: u32,
) -> NodeIndex {
    unimplemented!("Phase 1.1a: emit Kind 0x97")
}

/// `TypeImport` (Kind `0x98`, Pattern 2; 1 child = element).
pub fn write_type_import(
    _writer: &mut BinaryWriter<'_>,
    _span: Span,
    _parent_index: NodeIndex,
    _children_bitmask: u32,
) -> NodeIndex {
    unimplemented!("Phase 1.1a: emit Kind 0x98")
}

/// `TypePredicate` (Kind `0x99`, Pattern 2; 2 children = left + right).
pub fn write_type_predicate(
    _writer: &mut BinaryWriter<'_>,
    _span: Span,
    _parent_index: NodeIndex,
    _children_bitmask: u32,
) -> NodeIndex {
    unimplemented!("Phase 1.1a: emit Kind 0x99")
}

/// `TypeAsserts` (Kind `0x9A`, Pattern 2; 2 children = left + right).
pub fn write_type_asserts(
    _writer: &mut BinaryWriter<'_>,
    _span: Span,
    _parent_index: NodeIndex,
    _children_bitmask: u32,
) -> NodeIndex {
    unimplemented!("Phase 1.1a: emit Kind 0x9A")
}

/// `TypeAssertsPlain` (Kind `0x9B`, Pattern 2; 1 child = element).
pub fn write_type_asserts_plain(
    _writer: &mut BinaryWriter<'_>,
    _span: Span,
    _parent_index: NodeIndex,
    _children_bitmask: u32,
) -> NodeIndex {
    unimplemented!("Phase 1.1a: emit Kind 0x9B")
}

/// `TypeReadonlyArray` (Kind `0x9C`, Pattern 2; 1 child).
pub fn write_type_readonly_array(
    _writer: &mut BinaryWriter<'_>,
    _span: Span,
    _parent_index: NodeIndex,
    _children_bitmask: u32,
) -> NodeIndex {
    unimplemented!("Phase 1.1a: emit Kind 0x9C")
}

/// `TypeObjectField` (Kind `0xA0`, Pattern 2; 1-2 children).
///
/// Common Data: `bit0=optional`, `bit1=readonly`, `bits[2:3]=quote`.
pub fn write_type_object_field(
    _writer: &mut BinaryWriter<'_>,
    _span: Span,
    _parent_index: NodeIndex,
    _optional: bool,
    _readonly: bool,
    _quote: u8,
    _children_bitmask: u32,
) -> NodeIndex {
    unimplemented!("Phase 1.1a: emit Kind 0xA0")
}

/// `TypeJsdocObjectField` (Kind `0xA1`, Pattern 2; 2 children).
pub fn write_type_jsdoc_object_field(
    _writer: &mut BinaryWriter<'_>,
    _span: Span,
    _parent_index: NodeIndex,
    _children_bitmask: u32,
) -> NodeIndex {
    unimplemented!("Phase 1.1a: emit Kind 0xA1")
}

/// `TypeIndexedAccessIndex` (Kind `0xAA`, Pattern 2; 1 child).
pub fn write_type_indexed_access_index(
    _writer: &mut BinaryWriter<'_>,
    _span: Span,
    _parent_index: NodeIndex,
    _children_bitmask: u32,
) -> NodeIndex {
    unimplemented!("Phase 1.1a: emit Kind 0xAA")
}

/// `TypeCallSignature` (Kind `0xA7`, Pattern 2; 3 children).
pub fn write_type_call_signature(
    _writer: &mut BinaryWriter<'_>,
    _span: Span,
    _parent_index: NodeIndex,
    _children_bitmask: u32,
) -> NodeIndex {
    unimplemented!("Phase 1.1a: emit Kind 0xA7")
}

/// `TypeConstructorSignature` (Kind `0xA8`, Pattern 2; 3 children).
pub fn write_type_constructor_signature(
    _writer: &mut BinaryWriter<'_>,
    _span: Span,
    _parent_index: NodeIndex,
    _children_bitmask: u32,
) -> NodeIndex {
    unimplemented!("Phase 1.1a: emit Kind 0xA8")
}

/// `TypeTypeParameter` (Kind `0xA6`, Pattern 2; variable children).
pub fn write_type_type_parameter(
    _writer: &mut BinaryWriter<'_>,
    _span: Span,
    _parent_index: NodeIndex,
    _children_bitmask: u32,
) -> NodeIndex {
    unimplemented!("Phase 1.1a: emit Kind 0xA6")
}

/// `TypeParameterList` (Kind `0xAB`, Pattern 2; child = NodeList).
pub fn write_type_parameter_list(
    _writer: &mut BinaryWriter<'_>,
    _span: Span,
    _parent_index: NodeIndex,
    _children_bitmask: u32,
) -> NodeIndex {
    unimplemented!("Phase 1.1a: emit Kind 0xAB")
}

/// `TypeReadonlyProperty` (Kind `0xAC`, Pattern 2; 1 child).
pub fn write_type_readonly_property(
    _writer: &mut BinaryWriter<'_>,
    _span: Span,
    _parent_index: NodeIndex,
    _children_bitmask: u32,
) -> NodeIndex {
    unimplemented!("Phase 1.1a: emit Kind 0xAC")
}

// ===========================================================================
// Pattern 3: Mixed string + children (6 kinds, payload = Extended Data offset)
// ===========================================================================

/// `TypeKeyValue` (Kind `0xA2`, Pattern 3).
///
/// Common Data: `bit0=optional`, `bit1=variadic`. Extended Data: 2 bytes
/// (key string index). Children: 0 or 1.
pub fn write_type_key_value(
    _writer: &mut BinaryWriter<'_>,
    _span: Span,
    _parent_index: NodeIndex,
    _optional: bool,
    _variadic: bool,
    _key: StringIndex,
) -> NodeIndex {
    unimplemented!("Phase 1.1a: emit Kind 0xA2")
}

/// `TypeIndexSignature` (Kind `0xA4`, Pattern 3; 1 child = right).
pub fn write_type_index_signature(
    _writer: &mut BinaryWriter<'_>,
    _span: Span,
    _parent_index: NodeIndex,
    _key: StringIndex,
) -> NodeIndex {
    unimplemented!("Phase 1.1a: emit Kind 0xA4")
}

/// `TypeMappedType` (Kind `0xA5`, Pattern 3; 1 child = right).
pub fn write_type_mapped_type(
    _writer: &mut BinaryWriter<'_>,
    _span: Span,
    _parent_index: NodeIndex,
    _key: StringIndex,
) -> NodeIndex {
    unimplemented!("Phase 1.1a: emit Kind 0xA5")
}

/// `TypeMethodSignature` (Kind `0xA9`, Pattern 3).
///
/// Common Data: `bits[0:1]=quote`, `bit2=has_parameters`,
/// `bit3=has_type_parameters`. Children: 1 - 3.
pub fn write_type_method_signature(
    _writer: &mut BinaryWriter<'_>,
    _span: Span,
    _parent_index: NodeIndex,
    _quote: u8,
    _has_parameters: bool,
    _has_type_parameters: bool,
    _name: StringIndex,
) -> NodeIndex {
    unimplemented!("Phase 1.1a: emit Kind 0xA9")
}

/// `TypeTemplateLiteral` (Kind `0x9D`, Pattern 3; 1 NodeList child).
///
/// Extended Data: `2 + 2N` bytes (literal count + N string indices).
pub fn write_type_template_literal(
    _writer: &mut BinaryWriter<'_>,
    _span: Span,
    _parent_index: NodeIndex,
    _literals: &[StringIndex],
) -> NodeIndex {
    unimplemented!("Phase 1.1a: emit Kind 0x9D")
}

/// `TypeSymbol` (Kind `0x9F`, Pattern 3; 0 or 1 child = element).
///
/// Common Data: `bit0 = has_element`.
pub fn write_type_symbol(
    _writer: &mut BinaryWriter<'_>,
    _span: Span,
    _parent_index: NodeIndex,
    _has_element: bool,
    _value: StringIndex,
) -> NodeIndex {
    unimplemented!("Phase 1.1a: emit Kind 0x9F")
}

// ===========================================================================
// Others: pure leaves (5 kinds, no payload)
// ===========================================================================

/// `TypeNull` (Kind `0x83`, leaf).
pub fn write_type_null(
    _writer: &mut BinaryWriter<'_>,
    _span: Span,
    _parent_index: NodeIndex,
) -> NodeIndex {
    unimplemented!("Phase 1.1a: emit Kind 0x83")
}

/// `TypeUndefined` (Kind `0x84`, leaf).
pub fn write_type_undefined(
    _writer: &mut BinaryWriter<'_>,
    _span: Span,
    _parent_index: NodeIndex,
) -> NodeIndex {
    unimplemented!("Phase 1.1a: emit Kind 0x84")
}

/// `TypeAny` (Kind `0x85`, leaf).
pub fn write_type_any(
    _writer: &mut BinaryWriter<'_>,
    _span: Span,
    _parent_index: NodeIndex,
) -> NodeIndex {
    unimplemented!("Phase 1.1a: emit Kind 0x85")
}

/// `TypeUnknown` (Kind `0x86`, leaf).
pub fn write_type_unknown(
    _writer: &mut BinaryWriter<'_>,
    _span: Span,
    _parent_index: NodeIndex,
) -> NodeIndex {
    unimplemented!("Phase 1.1a: emit Kind 0x86")
}

/// `TypeUniqueSymbol` (Kind `0x9E`, leaf).
pub fn write_type_unique_symbol(
    _writer: &mut BinaryWriter<'_>,
    _span: Span,
    _parent_index: NodeIndex,
) -> NodeIndex {
    unimplemented!("Phase 1.1a: emit Kind 0x9E")
}
