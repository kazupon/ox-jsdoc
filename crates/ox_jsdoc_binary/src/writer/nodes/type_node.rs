//! `write_*` helpers for the 45 TypeNode kinds (`0x80 - 0xAC`).
//!
//! TypeNodes split into 4 patterns (see `format.md` "Node catalog matrix"):
//!
//! - **Pattern 1 — String only** (5 kinds): payload is a 30-bit string index.
//! - **Pattern 2 — Children only** (29 kinds): payload is a 30-bit Children
//!   bitmask.
//! - **Pattern 3 — Mixed string + children** (6 kinds): payload is a
//!   30-bit Extended Data offset; Extended Data holds the per-Kind layout.
//! - **Others** (5 kinds): pure leaves with no payload (`TypeNull` / `TypeUndefined`
//!   / `TypeAny` / `TypeUnknown` / `TypeUniqueSymbol`).

use oxc_span::Span;

use crate::format::kind::Kind;
use crate::format::node_record::{TypeTag, pack_node_data};

use super::super::{BinaryWriter, StringIndex};
use super::NodeIndex;

// ===========================================================================
// Pattern 1: String only (5 kinds, payload = string index)
// ===========================================================================

/// `TypeName` (Kind `0x80`, Pattern 1).
pub fn write_type_name(
    writer: &mut BinaryWriter<'_>,
    span: Span,
    parent_index: u32,
    value: StringIndex,
) -> NodeIndex {
    writer.emit_string_node(parent_index, Kind::TypeName, 0, span, value)
}

/// `TypeNumber` (Kind `0x81`, Pattern 1).
pub fn write_type_number(
    writer: &mut BinaryWriter<'_>,
    span: Span,
    parent_index: u32,
    value: StringIndex,
) -> NodeIndex {
    writer.emit_string_node(parent_index, Kind::TypeNumber, 0, span, value)
}

/// `TypeStringValue` (Kind `0x82`, Pattern 1; Common Data: bits[0:1] = quote 3-state).
pub fn write_type_string_value(
    writer: &mut BinaryWriter<'_>,
    span: Span,
    parent_index: u32,
    quote: u8,
    value: StringIndex,
) -> NodeIndex {
    writer.emit_string_node(parent_index, Kind::TypeStringValue, quote & 0b11, span, value)
}

/// `TypeProperty` (Kind `0xA3`, Pattern 1; Common Data: bits[0:1] = quote 3-state).
pub fn write_type_property(
    writer: &mut BinaryWriter<'_>,
    span: Span,
    parent_index: u32,
    quote: u8,
    value: StringIndex,
) -> NodeIndex {
    writer.emit_string_node(parent_index, Kind::TypeProperty, quote & 0b11, span, value)
}

/// `TypeSpecialNamePath` (Kind `0x8F`, Pattern 1).
///
/// Common Data: `bits[0:1] = special_type` (3 variants) + `bits[2:3] = quote` (3-state).
pub fn write_type_special_name_path(
    writer: &mut BinaryWriter<'_>,
    span: Span,
    parent_index: u32,
    special_type: u8,
    quote: u8,
    value: StringIndex,
) -> NodeIndex {
    let common = (special_type & 0b11) | ((quote & 0b11) << 2);
    writer.emit_string_node(parent_index, Kind::TypeSpecialNamePath, common, span, value)
}

// ===========================================================================
// Pattern 2: Children only (29 kinds, payload = Children bitmask)
// ===========================================================================

/// `TypeUnion` (Kind `0x87`, Pattern 2; child = NodeList).
pub fn write_type_union(
    writer: &mut BinaryWriter<'_>,
    span: Span,
    parent_index: u32,
    children_bitmask: u32,
) -> NodeIndex {
    writer.emit_children_node(parent_index, Kind::TypeUnion, 0, span, children_bitmask)
}

/// `TypeIntersection` (Kind `0x88`, Pattern 2; child = NodeList).
pub fn write_type_intersection(
    writer: &mut BinaryWriter<'_>,
    span: Span,
    parent_index: u32,
    children_bitmask: u32,
) -> NodeIndex {
    writer.emit_children_node(parent_index, Kind::TypeIntersection, 0, span, children_bitmask)
}

/// `TypeGeneric` (Kind `0x89`, Pattern 2).
///
/// Common Data: `bit0 = brackets`, `bit1 = dot`. Children: 1 + NodeList.
pub fn write_type_generic(
    writer: &mut BinaryWriter<'_>,
    span: Span,
    parent_index: u32,
    brackets: u8,
    dot: bool,
    children_bitmask: u32,
) -> NodeIndex {
    let common = (brackets & 1) | ((dot as u8) << 1);
    writer.emit_children_node(parent_index, Kind::TypeGeneric, common, span, children_bitmask)
}

/// `TypeFunction` (Kind `0x8A`, Pattern 2).
///
/// Common Data: `bit0=constructor`, `bit1=arrow`, `bit2=parenthesis`.
pub fn write_type_function(
    writer: &mut BinaryWriter<'_>,
    span: Span,
    parent_index: u32,
    constructor: bool,
    arrow: bool,
    parenthesis: bool,
    children_bitmask: u32,
) -> NodeIndex {
    let common = (constructor as u8) | ((arrow as u8) << 1) | ((parenthesis as u8) << 2);
    writer.emit_children_node(parent_index, Kind::TypeFunction, common, span, children_bitmask)
}

/// `TypeObject` (Kind `0x8B`, Pattern 2).
///
/// Common Data: `bits[0:2] = separator`. Child = NodeList.
pub fn write_type_object(
    writer: &mut BinaryWriter<'_>,
    span: Span,
    parent_index: u32,
    separator: u8,
    children_bitmask: u32,
) -> NodeIndex {
    writer.emit_children_node(parent_index, Kind::TypeObject, separator & 0b111, span, children_bitmask)
}

/// `TypeTuple` (Kind `0x8C`, Pattern 2; child = NodeList).
pub fn write_type_tuple(
    writer: &mut BinaryWriter<'_>,
    span: Span,
    parent_index: u32,
    children_bitmask: u32,
) -> NodeIndex {
    writer.emit_children_node(parent_index, Kind::TypeTuple, 0, span, children_bitmask)
}

/// `TypeParenthesis` (Kind `0x8D`, Pattern 2; 1 child).
pub fn write_type_parenthesis(
    writer: &mut BinaryWriter<'_>,
    span: Span,
    parent_index: u32,
    children_bitmask: u32,
) -> NodeIndex {
    writer.emit_children_node(parent_index, Kind::TypeParenthesis, 0, span, children_bitmask)
}

/// `TypeNamePath` (Kind `0x8E`, Pattern 2).
///
/// Common Data: `bits[0:1] = path_type` (4 variants). Children: 2 (left + right).
pub fn write_type_name_path(
    writer: &mut BinaryWriter<'_>,
    span: Span,
    parent_index: u32,
    path_type: u8,
    children_bitmask: u32,
) -> NodeIndex {
    writer.emit_children_node(parent_index, Kind::TypeNamePath, path_type & 0b11, span, children_bitmask)
}

/// `TypeNullable` (Kind `0x90`, Pattern 2; 1 child).
pub fn write_type_nullable(
    writer: &mut BinaryWriter<'_>,
    span: Span,
    parent_index: u32,
    position: u8,
    children_bitmask: u32,
) -> NodeIndex {
    writer.emit_children_node(parent_index, Kind::TypeNullable, position & 1, span, children_bitmask)
}

/// `TypeNotNullable` (Kind `0x91`, Pattern 2; 1 child).
pub fn write_type_not_nullable(
    writer: &mut BinaryWriter<'_>,
    span: Span,
    parent_index: u32,
    position: u8,
    children_bitmask: u32,
) -> NodeIndex {
    writer.emit_children_node(parent_index, Kind::TypeNotNullable, position & 1, span, children_bitmask)
}

/// `TypeOptional` (Kind `0x92`, Pattern 2; 1 child).
pub fn write_type_optional(
    writer: &mut BinaryWriter<'_>,
    span: Span,
    parent_index: u32,
    position: u8,
    children_bitmask: u32,
) -> NodeIndex {
    writer.emit_children_node(parent_index, Kind::TypeOptional, position & 1, span, children_bitmask)
}

/// `TypeVariadic` (Kind `0x93`, Pattern 2; 1 child).
pub fn write_type_variadic(
    writer: &mut BinaryWriter<'_>,
    span: Span,
    parent_index: u32,
    position: u8,
    square_brackets: bool,
    children_bitmask: u32,
) -> NodeIndex {
    let common = (position & 1) | ((square_brackets as u8) << 1);
    writer.emit_children_node(parent_index, Kind::TypeVariadic, common, span, children_bitmask)
}

/// `TypeConditional` (Kind `0x94`, Pattern 2; 4 children).
pub fn write_type_conditional(
    writer: &mut BinaryWriter<'_>,
    span: Span,
    parent_index: u32,
    children_bitmask: u32,
) -> NodeIndex {
    writer.emit_children_node(parent_index, Kind::TypeConditional, 0, span, children_bitmask)
}

/// `TypeInfer` (Kind `0x95`, Pattern 2; 1 child).
pub fn write_type_infer(
    writer: &mut BinaryWriter<'_>,
    span: Span,
    parent_index: u32,
    children_bitmask: u32,
) -> NodeIndex {
    writer.emit_children_node(parent_index, Kind::TypeInfer, 0, span, children_bitmask)
}

/// `TypeKeyOf` (Kind `0x96`, Pattern 2; 1 child).
pub fn write_type_key_of(
    writer: &mut BinaryWriter<'_>,
    span: Span,
    parent_index: u32,
    children_bitmask: u32,
) -> NodeIndex {
    writer.emit_children_node(parent_index, Kind::TypeKeyOf, 0, span, children_bitmask)
}

/// `TypeTypeOf` (Kind `0x97`, Pattern 2; 1 child).
pub fn write_type_type_of(
    writer: &mut BinaryWriter<'_>,
    span: Span,
    parent_index: u32,
    children_bitmask: u32,
) -> NodeIndex {
    writer.emit_children_node(parent_index, Kind::TypeTypeOf, 0, span, children_bitmask)
}

/// `TypeImport` (Kind `0x98`, Pattern 2; 1 child = element).
pub fn write_type_import(
    writer: &mut BinaryWriter<'_>,
    span: Span,
    parent_index: u32,
    children_bitmask: u32,
) -> NodeIndex {
    writer.emit_children_node(parent_index, Kind::TypeImport, 0, span, children_bitmask)
}

/// `TypePredicate` (Kind `0x99`, Pattern 2; 2 children).
pub fn write_type_predicate(
    writer: &mut BinaryWriter<'_>,
    span: Span,
    parent_index: u32,
    children_bitmask: u32,
) -> NodeIndex {
    writer.emit_children_node(parent_index, Kind::TypePredicate, 0, span, children_bitmask)
}

/// `TypeAsserts` (Kind `0x9A`, Pattern 2; 2 children).
pub fn write_type_asserts(
    writer: &mut BinaryWriter<'_>,
    span: Span,
    parent_index: u32,
    children_bitmask: u32,
) -> NodeIndex {
    writer.emit_children_node(parent_index, Kind::TypeAsserts, 0, span, children_bitmask)
}

/// `TypeAssertsPlain` (Kind `0x9B`, Pattern 2; 1 child).
pub fn write_type_asserts_plain(
    writer: &mut BinaryWriter<'_>,
    span: Span,
    parent_index: u32,
    children_bitmask: u32,
) -> NodeIndex {
    writer.emit_children_node(parent_index, Kind::TypeAssertsPlain, 0, span, children_bitmask)
}

/// `TypeReadonlyArray` (Kind `0x9C`, Pattern 2; 1 child).
pub fn write_type_readonly_array(
    writer: &mut BinaryWriter<'_>,
    span: Span,
    parent_index: u32,
    children_bitmask: u32,
) -> NodeIndex {
    writer.emit_children_node(parent_index, Kind::TypeReadonlyArray, 0, span, children_bitmask)
}

/// `TypeObjectField` (Kind `0xA0`, Pattern 2; 1-2 children).
///
/// Common Data: `bit0=optional`, `bit1=readonly`, `bits[2:3]=quote (3-state)`.
pub fn write_type_object_field(
    writer: &mut BinaryWriter<'_>,
    span: Span,
    parent_index: u32,
    optional: bool,
    readonly: bool,
    quote: u8,
    children_bitmask: u32,
) -> NodeIndex {
    let common = (optional as u8) | ((readonly as u8) << 1) | ((quote & 0b11) << 2);
    writer.emit_children_node(parent_index, Kind::TypeObjectField, common, span, children_bitmask)
}

/// `TypeJsdocObjectField` (Kind `0xA1`, Pattern 2; 2 children).
pub fn write_type_jsdoc_object_field(
    writer: &mut BinaryWriter<'_>,
    span: Span,
    parent_index: u32,
    children_bitmask: u32,
) -> NodeIndex {
    writer.emit_children_node(parent_index, Kind::TypeJsdocObjectField, 0, span, children_bitmask)
}

/// `TypeIndexedAccessIndex` (Kind `0xAA`, Pattern 2; 1 child).
pub fn write_type_indexed_access_index(
    writer: &mut BinaryWriter<'_>,
    span: Span,
    parent_index: u32,
    children_bitmask: u32,
) -> NodeIndex {
    writer.emit_children_node(parent_index, Kind::TypeIndexedAccessIndex, 0, span, children_bitmask)
}

/// `TypeCallSignature` (Kind `0xA7`, Pattern 2; 3 children).
pub fn write_type_call_signature(
    writer: &mut BinaryWriter<'_>,
    span: Span,
    parent_index: u32,
    children_bitmask: u32,
) -> NodeIndex {
    writer.emit_children_node(parent_index, Kind::TypeCallSignature, 0, span, children_bitmask)
}

/// `TypeConstructorSignature` (Kind `0xA8`, Pattern 2; 3 children).
pub fn write_type_constructor_signature(
    writer: &mut BinaryWriter<'_>,
    span: Span,
    parent_index: u32,
    children_bitmask: u32,
) -> NodeIndex {
    writer.emit_children_node(parent_index, Kind::TypeConstructorSignature, 0, span, children_bitmask)
}

/// `TypeTypeParameter` (Kind `0xA6`, Pattern 2; variable children).
pub fn write_type_type_parameter(
    writer: &mut BinaryWriter<'_>,
    span: Span,
    parent_index: u32,
    children_bitmask: u32,
) -> NodeIndex {
    writer.emit_children_node(parent_index, Kind::TypeTypeParameter, 0, span, children_bitmask)
}

/// `TypeParameterList` (Kind `0xAB`, Pattern 2; child = NodeList).
pub fn write_type_parameter_list(
    writer: &mut BinaryWriter<'_>,
    span: Span,
    parent_index: u32,
    children_bitmask: u32,
) -> NodeIndex {
    writer.emit_children_node(parent_index, Kind::TypeParameterList, 0, span, children_bitmask)
}

/// `TypeReadonlyProperty` (Kind `0xAC`, Pattern 2; 1 child).
pub fn write_type_readonly_property(
    writer: &mut BinaryWriter<'_>,
    span: Span,
    parent_index: u32,
    children_bitmask: u32,
) -> NodeIndex {
    writer.emit_children_node(parent_index, Kind::TypeReadonlyProperty, 0, span, children_bitmask)
}

// ===========================================================================
// Pattern 3: Mixed string + children (6 kinds, payload = Extended Data offset)
//
// Each helper:
// 1. Reserves Extended Data of the per-Kind size.
// 2. Writes the per-Kind layout (typically u16 string indices).
// 3. Emits the node record with `TypeTag::Extended` payload pointing to the
//    reserved offset.
// ===========================================================================

/// `TypeKeyValue` (Kind `0xA2`, Pattern 3; 2 bytes Extended Data = key index).
///
/// Common Data: `bit0 = optional`, `bit1 = variadic`.
pub fn write_type_key_value(
    writer: &mut BinaryWriter<'_>,
    span: Span,
    parent_index: u32,
    optional: bool,
    variadic: bool,
    key: StringIndex,
) -> NodeIndex {
    let common = (optional as u8) | ((variadic as u8) << 1);
    let off = writer.extended.reserve(2);
    writer.extended.slice_mut(off, 2).copy_from_slice(&key.as_u16().to_le_bytes());
    writer.emit_extended_node(parent_index, Kind::TypeKeyValue, common, span, off)
}

/// `TypeIndexSignature` (Kind `0xA4`, Pattern 3; 1 child).
pub fn write_type_index_signature(
    writer: &mut BinaryWriter<'_>,
    span: Span,
    parent_index: u32,
    key: StringIndex,
) -> NodeIndex {
    let off = writer.extended.reserve(2);
    writer.extended.slice_mut(off, 2).copy_from_slice(&key.as_u16().to_le_bytes());
    writer.emit_extended_node(parent_index, Kind::TypeIndexSignature, 0, span, off)
}

/// `TypeMappedType` (Kind `0xA5`, Pattern 3; 1 child).
pub fn write_type_mapped_type(
    writer: &mut BinaryWriter<'_>,
    span: Span,
    parent_index: u32,
    key: StringIndex,
) -> NodeIndex {
    let off = writer.extended.reserve(2);
    writer.extended.slice_mut(off, 2).copy_from_slice(&key.as_u16().to_le_bytes());
    writer.emit_extended_node(parent_index, Kind::TypeMappedType, 0, span, off)
}

/// `TypeMethodSignature` (Kind `0xA9`, Pattern 3).
///
/// Common Data: `bits[0:1]=quote`, `bit2=has_parameters`, `bit3=has_type_parameters`.
pub fn write_type_method_signature(
    writer: &mut BinaryWriter<'_>,
    span: Span,
    parent_index: u32,
    quote: u8,
    has_parameters: bool,
    has_type_parameters: bool,
    name: StringIndex,
) -> NodeIndex {
    let common = (quote & 0b11)
        | ((has_parameters as u8) << 2)
        | ((has_type_parameters as u8) << 3);
    let off = writer.extended.reserve(2);
    writer.extended.slice_mut(off, 2).copy_from_slice(&name.as_u16().to_le_bytes());
    writer.emit_extended_node(parent_index, Kind::TypeMethodSignature, common, span, off)
}

/// `TypeTemplateLiteral` (Kind `0x9D`, Pattern 3; 1 NodeList child).
///
/// Extended Data: `2 + 2N` bytes (`u16 literal_count` + `N × u16` indices).
pub fn write_type_template_literal(
    writer: &mut BinaryWriter<'_>,
    span: Span,
    parent_index: u32,
    literals: &[StringIndex],
) -> NodeIndex {
    let n = literals.len();
    let size = 2 + 2 * n;
    let off = writer.extended.reserve(size);
    let len_u16 = u16::try_from(n).expect("literal count exceeds u16");
    writer
        .extended
        .slice_mut(off, 2)
        .copy_from_slice(&len_u16.to_le_bytes());
    let dst = writer.extended.slice_mut(off, size);
    for (i, lit) in literals.iter().enumerate() {
        let pos = 2 + i * 2;
        dst[pos..pos + 2].copy_from_slice(&lit.as_u16().to_le_bytes());
    }
    writer.emit_extended_node(parent_index, Kind::TypeTemplateLiteral, 0, span, off)
}

/// `TypeSymbol` (Kind `0x9F`, Pattern 3; 0 or 1 child).
///
/// Common Data: `bit0 = has_element`.
pub fn write_type_symbol(
    writer: &mut BinaryWriter<'_>,
    span: Span,
    parent_index: u32,
    has_element: bool,
    value: StringIndex,
) -> NodeIndex {
    let common = has_element as u8;
    let off = writer.extended.reserve(2);
    writer.extended.slice_mut(off, 2).copy_from_slice(&value.as_u16().to_le_bytes());
    writer.emit_extended_node(parent_index, Kind::TypeSymbol, common, span, off)
}

// ===========================================================================
// Others: pure leaves (5 kinds, no payload)
// ===========================================================================

/// `TypeNull` (Kind `0x83`, leaf).
pub fn write_type_null(
    writer: &mut BinaryWriter<'_>,
    span: Span,
    parent_index: u32,
) -> NodeIndex {
    let node_data = pack_node_data(TypeTag::Children, 0);
    writer.emit_node_record(parent_index, Kind::TypeNull, 0, span, node_data)
}

/// `TypeUndefined` (Kind `0x84`, leaf).
pub fn write_type_undefined(
    writer: &mut BinaryWriter<'_>,
    span: Span,
    parent_index: u32,
) -> NodeIndex {
    let node_data = pack_node_data(TypeTag::Children, 0);
    writer.emit_node_record(parent_index, Kind::TypeUndefined, 0, span, node_data)
}

/// `TypeAny` (Kind `0x85`, leaf).
pub fn write_type_any(
    writer: &mut BinaryWriter<'_>,
    span: Span,
    parent_index: u32,
) -> NodeIndex {
    let node_data = pack_node_data(TypeTag::Children, 0);
    writer.emit_node_record(parent_index, Kind::TypeAny, 0, span, node_data)
}

/// `TypeUnknown` (Kind `0x86`, leaf).
pub fn write_type_unknown(
    writer: &mut BinaryWriter<'_>,
    span: Span,
    parent_index: u32,
) -> NodeIndex {
    let node_data = pack_node_data(TypeTag::Children, 0);
    writer.emit_node_record(parent_index, Kind::TypeUnknown, 0, span, node_data)
}

/// `TypeUniqueSymbol` (Kind `0x9E`, leaf).
pub fn write_type_unique_symbol(
    writer: &mut BinaryWriter<'_>,
    span: Span,
    parent_index: u32,
) -> NodeIndex {
    let node_data = pack_node_data(TypeTag::Children, 0);
    writer.emit_node_record(parent_index, Kind::TypeUniqueSymbol, 0, span, node_data)
}
