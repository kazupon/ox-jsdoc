//! Lazy structs for the 45 TypeNode kinds (`0x80 - 0xAC`).
//!
//! Each struct mirrors the comment-AST style in
//! [`super::comment_ast`]: a `Copy` value type holding
//! `(source_file, node_index)` plus per-Kind getters that are `todo!()` in
//! Phase 1.0c.
//!
//! In addition to the per-Kind structs, this module exposes the
//! [`LazyTypeNode`] enum for callers (such as `JsdocTag.parsed_type`) that
//! receive a TypeNode of unknown variant.

use crate::format::kind::Kind;

use super::super::source_file::LazySourceFile;
use super::{LazyNode, NodeListIter};

/// Generate a lazy TypeNode struct + its `LazyNode` impl in one go.
macro_rules! define_lazy_type_node {
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

// ===========================================================================
// Pattern 1 — String only (5 kinds, payload = string index)
// ===========================================================================
define_lazy_type_node!(LazyTypeName, Kind::TypeName, "Lazy view of `TypeName` (Kind 0x80).");
impl<'a> LazyTypeName<'a> {
    /// Identifier name string.
    pub fn value(&self) -> &'a str { todo!("Phase 1.1b: 30-bit String payload") }
}

define_lazy_type_node!(LazyTypeNumber, Kind::TypeNumber, "Lazy view of `TypeNumber` (Kind 0x81).");
impl<'a> LazyTypeNumber<'a> {
    /// Numeric literal as written in the source.
    pub fn value(&self) -> &'a str { todo!("Phase 1.1b: 30-bit String payload") }
}

define_lazy_type_node!(
    LazyTypeStringValue,
    Kind::TypeStringValue,
    "Lazy view of `TypeStringValue` (Kind 0x82)."
);
impl<'a> LazyTypeStringValue<'a> {
    /// Quote style (None=0 / Single=1 / Double=2). Encoder always writes 1 or 2.
    pub fn quote(&self) -> u8 { todo!("Phase 1.1b: Common Data bits[0:1]") }
    /// String literal value (without the surrounding quotes).
    pub fn value(&self) -> &'a str { todo!("Phase 1.1b: 30-bit String payload") }
}

define_lazy_type_node!(LazyTypeProperty, Kind::TypeProperty, "Lazy view of `TypeProperty` (Kind 0xA3).");
impl<'a> LazyTypeProperty<'a> {
    /// Quote style (3-state).
    pub fn quote(&self) -> u8 { todo!("Phase 1.1b: Common Data bits[0:1]") }
    /// Property name string.
    pub fn value(&self) -> &'a str { todo!("Phase 1.1b: 30-bit String payload") }
}

define_lazy_type_node!(
    LazyTypeSpecialNamePath,
    Kind::TypeSpecialNamePath,
    "Lazy view of `TypeSpecialNamePath` (Kind 0x8F)."
);
impl<'a> LazyTypeSpecialNamePath<'a> {
    /// Special path category (3 variants stored in Common Data bits[0:1]).
    pub fn special_type(&self) -> u8 { todo!("Phase 1.1b: Common Data bits[0:1]") }
    /// Quote style (3-state stored in Common Data bits[2:3]).
    pub fn quote(&self) -> u8 { todo!("Phase 1.1b: Common Data bits[2:3]") }
    /// Path string.
    pub fn value(&self) -> &'a str { todo!("Phase 1.1b: 30-bit String payload") }
}

// ===========================================================================
// Pattern 2 — Children only (29 kinds, payload = bitmask)
// ===========================================================================
define_lazy_type_node!(LazyTypeUnion, Kind::TypeUnion, "Lazy view of `TypeUnion` (Kind 0x87).");
impl<'a> LazyTypeUnion<'a> {
    /// Union elements as a NodeList iterator.
    pub fn elements(&self) -> NodeListIter<'a, LazyTypeNode<'a>> { todo!("Phase 1.1b: visitor index 0 (NodeList)") }
}

define_lazy_type_node!(
    LazyTypeIntersection,
    Kind::TypeIntersection,
    "Lazy view of `TypeIntersection` (Kind 0x88)."
);
impl<'a> LazyTypeIntersection<'a> {
    /// Intersection elements as a NodeList iterator.
    pub fn elements(&self) -> NodeListIter<'a, LazyTypeNode<'a>> { todo!("Phase 1.1b: visitor index 0") }
}

define_lazy_type_node!(LazyTypeGeneric, Kind::TypeGeneric, "Lazy view of `TypeGeneric` (Kind 0x89).");
impl<'a> LazyTypeGeneric<'a> {
    /// Bracket style (Angle / Square). Stored in Common Data bit0.
    pub fn brackets(&self) -> u8 { todo!("Phase 1.1b: Common Data bit0") }
    /// Whether the generic was written with a leading `.` (Closure form).
    pub fn dot(&self) -> bool { todo!("Phase 1.1b: Common Data bit1") }
    /// Left-hand side type.
    pub fn left(&self) -> LazyTypeNode<'a> { todo!("Phase 1.1b: visitor index 0") }
    /// Generic argument elements.
    pub fn elements(&self) -> NodeListIter<'a, LazyTypeNode<'a>> { todo!("Phase 1.1b: visitor index 1") }
}

define_lazy_type_node!(LazyTypeFunction, Kind::TypeFunction, "Lazy view of `TypeFunction` (Kind 0x8A).");
impl<'a> LazyTypeFunction<'a> {
    /// Constructor flag (`new () => T`). Stored in Common Data bit0.
    pub fn constructor(&self) -> bool { todo!("Phase 1.1b: Common Data bit0") }
    /// Arrow form flag.
    pub fn arrow(&self) -> bool { todo!("Phase 1.1b: Common Data bit1") }
    /// Whether parameters were enclosed in parentheses.
    pub fn parenthesis(&self) -> bool { todo!("Phase 1.1b: Common Data bit2") }
    /// Parameter list child.
    pub fn parameters(&self) -> Option<LazyTypeParameterList<'a>> { todo!("Phase 1.1b: visitor index 0") }
    /// Return type child.
    pub fn return_type(&self) -> Option<LazyTypeNode<'a>> { todo!("Phase 1.1b: visitor index 1") }
    /// Type-parameter list child (`<T>`).
    pub fn type_parameters(&self) -> Option<LazyTypeParameterList<'a>> { todo!("Phase 1.1b: visitor index 2") }
}

define_lazy_type_node!(LazyTypeObject, Kind::TypeObject, "Lazy view of `TypeObject` (Kind 0x8B).");
impl<'a> LazyTypeObject<'a> {
    /// Field separator style (Common Data bits[0:2]).
    pub fn separator(&self) -> u8 { todo!("Phase 1.1b: Common Data bits[0:2]") }
    /// Field elements as a NodeList iterator.
    pub fn elements(&self) -> NodeListIter<'a, LazyTypeNode<'a>> { todo!("Phase 1.1b: visitor index 0") }
}

define_lazy_type_node!(LazyTypeTuple, Kind::TypeTuple, "Lazy view of `TypeTuple` (Kind 0x8C).");
impl<'a> LazyTypeTuple<'a> {
    /// Tuple elements as a NodeList iterator.
    pub fn elements(&self) -> NodeListIter<'a, LazyTypeNode<'a>> { todo!("Phase 1.1b: visitor index 0") }
}

define_lazy_type_node!(
    LazyTypeParenthesis,
    Kind::TypeParenthesis,
    "Lazy view of `TypeParenthesis` (Kind 0x8D)."
);
impl<'a> LazyTypeParenthesis<'a> {
    /// Wrapped type.
    pub fn element(&self) -> LazyTypeNode<'a> { todo!("Phase 1.1b: visitor index 0") }
}

define_lazy_type_node!(LazyTypeNamePath, Kind::TypeNamePath, "Lazy view of `TypeNamePath` (Kind 0x8E).");
impl<'a> LazyTypeNamePath<'a> {
    /// Path connector category (4 variants in Common Data bits[0:1]).
    pub fn path_type(&self) -> u8 { todo!("Phase 1.1b: Common Data bits[0:1]") }
    /// Left-hand side.
    pub fn left(&self) -> LazyTypeNode<'a> { todo!("Phase 1.1b: visitor index 0") }
    /// Right-hand side.
    pub fn right(&self) -> LazyTypeNode<'a> { todo!("Phase 1.1b: visitor index 1") }
}

define_lazy_type_node!(LazyTypeNullable, Kind::TypeNullable, "Lazy view of `TypeNullable` (Kind 0x90).");
impl<'a> LazyTypeNullable<'a> {
    /// Modifier position (Prefix=0 / Suffix=1).
    pub fn position(&self) -> u8 { todo!("Phase 1.1b: Common Data bit0") }
    /// Wrapped type.
    pub fn element(&self) -> LazyTypeNode<'a> { todo!("Phase 1.1b: visitor index 0") }
}

define_lazy_type_node!(
    LazyTypeNotNullable,
    Kind::TypeNotNullable,
    "Lazy view of `TypeNotNullable` (Kind 0x91)."
);
impl<'a> LazyTypeNotNullable<'a> {
    /// Modifier position (Prefix=0 / Suffix=1).
    pub fn position(&self) -> u8 { todo!("Phase 1.1b: Common Data bit0") }
    /// Wrapped type.
    pub fn element(&self) -> LazyTypeNode<'a> { todo!("Phase 1.1b: visitor index 0") }
}

define_lazy_type_node!(LazyTypeOptional, Kind::TypeOptional, "Lazy view of `TypeOptional` (Kind 0x92).");
impl<'a> LazyTypeOptional<'a> {
    /// Modifier position (Prefix=0 / Suffix=1).
    pub fn position(&self) -> u8 { todo!("Phase 1.1b: Common Data bit0") }
    /// Wrapped type.
    pub fn element(&self) -> LazyTypeNode<'a> { todo!("Phase 1.1b: visitor index 0") }
}

define_lazy_type_node!(LazyTypeVariadic, Kind::TypeVariadic, "Lazy view of `TypeVariadic` (Kind 0x93).");
impl<'a> LazyTypeVariadic<'a> {
    /// Modifier position.
    pub fn position(&self) -> u8 { todo!("Phase 1.1b: Common Data bit0") }
    /// Whether the variadic was written with `[]` brackets.
    pub fn square_brackets(&self) -> bool { todo!("Phase 1.1b: Common Data bit1") }
    /// Wrapped type.
    pub fn element(&self) -> LazyTypeNode<'a> { todo!("Phase 1.1b: visitor index 0") }
}

define_lazy_type_node!(
    LazyTypeConditional,
    Kind::TypeConditional,
    "Lazy view of `TypeConditional` (Kind 0x94)."
);
impl<'a> LazyTypeConditional<'a> {
    /// `T` in `T extends U ? X : Y`.
    pub fn check_type(&self) -> LazyTypeNode<'a> { todo!("Phase 1.1b: visitor index 0") }
    /// `U` in `T extends U ? X : Y`.
    pub fn extends_type(&self) -> LazyTypeNode<'a> { todo!("Phase 1.1b: visitor index 1") }
    /// `X` (true branch).
    pub fn true_type(&self) -> LazyTypeNode<'a> { todo!("Phase 1.1b: visitor index 2") }
    /// `Y` (false branch).
    pub fn false_type(&self) -> LazyTypeNode<'a> { todo!("Phase 1.1b: visitor index 3") }
}

define_lazy_type_node!(LazyTypeInfer, Kind::TypeInfer, "Lazy view of `TypeInfer` (Kind 0x95).");
impl<'a> LazyTypeInfer<'a> {
    /// Inferred type parameter.
    pub fn element(&self) -> LazyTypeNode<'a> { todo!("Phase 1.1b: visitor index 0") }
}

define_lazy_type_node!(LazyTypeKeyOf, Kind::TypeKeyOf, "Lazy view of `TypeKeyOf` (Kind 0x96).");
impl<'a> LazyTypeKeyOf<'a> {
    /// Operand type.
    pub fn element(&self) -> LazyTypeNode<'a> { todo!("Phase 1.1b: visitor index 0") }
}

define_lazy_type_node!(LazyTypeTypeOf, Kind::TypeTypeOf, "Lazy view of `TypeTypeOf` (Kind 0x97).");
impl<'a> LazyTypeTypeOf<'a> {
    /// Operand type.
    pub fn element(&self) -> LazyTypeNode<'a> { todo!("Phase 1.1b: visitor index 0") }
}

define_lazy_type_node!(LazyTypeImport, Kind::TypeImport, "Lazy view of `TypeImport` (Kind 0x98).");
impl<'a> LazyTypeImport<'a> {
    /// Imported type spec.
    pub fn element(&self) -> LazyTypeNode<'a> { todo!("Phase 1.1b: visitor index 0") }
}

define_lazy_type_node!(LazyTypePredicate, Kind::TypePredicate, "Lazy view of `TypePredicate` (Kind 0x99).");
impl<'a> LazyTypePredicate<'a> {
    /// Predicate parameter (left side).
    pub fn left(&self) -> LazyTypeNode<'a> { todo!("Phase 1.1b: visitor index 0") }
    /// Predicate type (right side).
    pub fn right(&self) -> LazyTypeNode<'a> { todo!("Phase 1.1b: visitor index 1") }
}

define_lazy_type_node!(LazyTypeAsserts, Kind::TypeAsserts, "Lazy view of `TypeAsserts` (Kind 0x9A).");
impl<'a> LazyTypeAsserts<'a> {
    /// Asserts parameter (left).
    pub fn left(&self) -> LazyTypeNode<'a> { todo!("Phase 1.1b: visitor index 0") }
    /// Asserted type (right).
    pub fn right(&self) -> LazyTypeNode<'a> { todo!("Phase 1.1b: visitor index 1") }
}

define_lazy_type_node!(
    LazyTypeAssertsPlain,
    Kind::TypeAssertsPlain,
    "Lazy view of `TypeAssertsPlain` (Kind 0x9B)."
);
impl<'a> LazyTypeAssertsPlain<'a> {
    /// Asserts parameter.
    pub fn element(&self) -> LazyTypeNode<'a> { todo!("Phase 1.1b: visitor index 0") }
}

define_lazy_type_node!(
    LazyTypeReadonlyArray,
    Kind::TypeReadonlyArray,
    "Lazy view of `TypeReadonlyArray` (Kind 0x9C)."
);
impl<'a> LazyTypeReadonlyArray<'a> {
    /// Element type.
    pub fn element(&self) -> LazyTypeNode<'a> { todo!("Phase 1.1b: visitor index 0") }
}

define_lazy_type_node!(
    LazyTypeObjectField,
    Kind::TypeObjectField,
    "Lazy view of `TypeObjectField` (Kind 0xA0)."
);
impl<'a> LazyTypeObjectField<'a> {
    /// Optional `?` modifier flag.
    pub fn optional(&self) -> bool { todo!("Phase 1.1b: Common Data bit0") }
    /// Readonly modifier flag.
    pub fn readonly(&self) -> bool { todo!("Phase 1.1b: Common Data bit1") }
    /// Quote style for the field key (3-state).
    pub fn quote(&self) -> u8 { todo!("Phase 1.1b: Common Data bits[2:3]") }
    /// Field key.
    pub fn key(&self) -> LazyTypeNode<'a> { todo!("Phase 1.1b: visitor index 0") }
    /// Field value type.
    pub fn right(&self) -> Option<LazyTypeNode<'a>> { todo!("Phase 1.1b: visitor index 1") }
}

define_lazy_type_node!(
    LazyTypeJsdocObjectField,
    Kind::TypeJsdocObjectField,
    "Lazy view of `TypeJsdocObjectField` (Kind 0xA1)."
);
impl<'a> LazyTypeJsdocObjectField<'a> {
    /// Field key.
    pub fn key(&self) -> LazyTypeNode<'a> { todo!("Phase 1.1b: visitor index 0") }
    /// Field value type.
    pub fn right(&self) -> LazyTypeNode<'a> { todo!("Phase 1.1b: visitor index 1") }
}

define_lazy_type_node!(
    LazyTypeIndexedAccessIndex,
    Kind::TypeIndexedAccessIndex,
    "Lazy view of `TypeIndexedAccessIndex` (Kind 0xAA)."
);
impl<'a> LazyTypeIndexedAccessIndex<'a> {
    /// Indexed type.
    pub fn element(&self) -> LazyTypeNode<'a> { todo!("Phase 1.1b: visitor index 0") }
}

define_lazy_type_node!(
    LazyTypeCallSignature,
    Kind::TypeCallSignature,
    "Lazy view of `TypeCallSignature` (Kind 0xA7)."
);
impl<'a> LazyTypeCallSignature<'a> {
    /// Parameter list.
    pub fn parameters(&self) -> Option<LazyTypeParameterList<'a>> { todo!("Phase 1.1b: visitor index 0") }
    /// Return type.
    pub fn return_type(&self) -> LazyTypeNode<'a> { todo!("Phase 1.1b: visitor index 1") }
    /// Type-parameter list.
    pub fn type_parameters(&self) -> Option<LazyTypeParameterList<'a>> { todo!("Phase 1.1b: visitor index 2") }
}

define_lazy_type_node!(
    LazyTypeConstructorSignature,
    Kind::TypeConstructorSignature,
    "Lazy view of `TypeConstructorSignature` (Kind 0xA8)."
);
impl<'a> LazyTypeConstructorSignature<'a> {
    /// Parameter list.
    pub fn parameters(&self) -> Option<LazyTypeParameterList<'a>> { todo!("Phase 1.1b: visitor index 0") }
    /// Return type.
    pub fn return_type(&self) -> LazyTypeNode<'a> { todo!("Phase 1.1b: visitor index 1") }
    /// Type-parameter list.
    pub fn type_parameters(&self) -> Option<LazyTypeParameterList<'a>> { todo!("Phase 1.1b: visitor index 2") }
}

define_lazy_type_node!(
    LazyTypeTypeParameter,
    Kind::TypeTypeParameter,
    "Lazy view of `TypeTypeParameter` (Kind 0xA6)."
);
impl<'a> LazyTypeTypeParameter<'a> {
    /// Type-parameter children.
    pub fn elements(&self) -> NodeListIter<'a, LazyTypeNode<'a>> { todo!("Phase 1.1b: visitor index 0") }
}

define_lazy_type_node!(
    LazyTypeParameterList,
    Kind::TypeParameterList,
    "Lazy view of `TypeParameterList` (Kind 0xAB)."
);
impl<'a> LazyTypeParameterList<'a> {
    /// Parameter list elements.
    pub fn elements(&self) -> NodeListIter<'a, LazyTypeNode<'a>> { todo!("Phase 1.1b: visitor index 0") }
}

define_lazy_type_node!(
    LazyTypeReadonlyProperty,
    Kind::TypeReadonlyProperty,
    "Lazy view of `TypeReadonlyProperty` (Kind 0xAC)."
);
impl<'a> LazyTypeReadonlyProperty<'a> {
    /// Wrapped property.
    pub fn element(&self) -> LazyTypeNode<'a> { todo!("Phase 1.1b: visitor index 0") }
}

// ===========================================================================
// Pattern 3 — Mixed string + children (6 kinds, payload = Extended Data offset)
// ===========================================================================
define_lazy_type_node!(LazyTypeKeyValue, Kind::TypeKeyValue, "Lazy view of `TypeKeyValue` (Kind 0xA2).");
impl<'a> LazyTypeKeyValue<'a> {
    /// Optional flag.
    pub fn optional(&self) -> bool { todo!("Phase 1.1b: Common Data bit0") }
    /// Variadic flag.
    pub fn variadic(&self) -> bool { todo!("Phase 1.1b: Common Data bit1") }
    /// Key string.
    pub fn key(&self) -> &'a str { todo!("Phase 1.1b: Extended Data byte 0-1") }
    /// Value type (when present).
    pub fn right(&self) -> Option<LazyTypeNode<'a>> { todo!("Phase 1.1b: visitor index 0") }
}

define_lazy_type_node!(
    LazyTypeIndexSignature,
    Kind::TypeIndexSignature,
    "Lazy view of `TypeIndexSignature` (Kind 0xA4)."
);
impl<'a> LazyTypeIndexSignature<'a> {
    /// Key string (e.g. `K` in `[K]: T`).
    pub fn key(&self) -> &'a str { todo!("Phase 1.1b: Extended Data byte 0-1") }
    /// Value type (`T`).
    pub fn right(&self) -> LazyTypeNode<'a> { todo!("Phase 1.1b: visitor index 0") }
}

define_lazy_type_node!(
    LazyTypeMappedType,
    Kind::TypeMappedType,
    "Lazy view of `TypeMappedType` (Kind 0xA5)."
);
impl<'a> LazyTypeMappedType<'a> {
    /// Key string.
    pub fn key(&self) -> &'a str { todo!("Phase 1.1b: Extended Data byte 0-1") }
    /// Value type.
    pub fn right(&self) -> LazyTypeNode<'a> { todo!("Phase 1.1b: visitor index 0") }
}

define_lazy_type_node!(
    LazyTypeMethodSignature,
    Kind::TypeMethodSignature,
    "Lazy view of `TypeMethodSignature` (Kind 0xA9)."
);
impl<'a> LazyTypeMethodSignature<'a> {
    /// Quote style for the method name.
    pub fn quote(&self) -> u8 { todo!("Phase 1.1b: Common Data bits[0:1]") }
    /// Whether parameters were emitted.
    pub fn has_parameters(&self) -> bool { todo!("Phase 1.1b: Common Data bit2") }
    /// Whether type-parameters were emitted.
    pub fn has_type_parameters(&self) -> bool { todo!("Phase 1.1b: Common Data bit3") }
    /// Method name string.
    pub fn name(&self) -> &'a str { todo!("Phase 1.1b: Extended Data byte 0-1") }
    /// Parameter list when `has_parameters()` is true.
    pub fn parameters(&self) -> Option<LazyTypeParameterList<'a>> { todo!("Phase 1.1b: optional first child") }
    /// Return type (always present).
    pub fn return_type(&self) -> LazyTypeNode<'a> { todo!("Phase 1.1b: required middle child") }
    /// Type-parameter list when `has_type_parameters()` is true.
    pub fn type_parameters(&self) -> Option<LazyTypeParameterList<'a>> {
        todo!("Phase 1.1b: optional last child")
    }
}

define_lazy_type_node!(
    LazyTypeTemplateLiteral,
    Kind::TypeTemplateLiteral,
    "Lazy view of `TypeTemplateLiteral` (Kind 0x9D)."
);
impl<'a> LazyTypeTemplateLiteral<'a> {
    /// Number of literal segments.
    pub fn literal_count(&self) -> u16 { todo!("Phase 1.1b: Extended Data byte 0-1") }
    /// Get the n-th literal segment by index.
    pub fn literal(&self, _index: u16) -> &'a str {
        todo!("Phase 1.1b: Extended Data byte (2 + index*2)..(+2)")
    }
    /// Interpolations (`${expr}`) as a NodeList.
    pub fn interpolations(&self) -> NodeListIter<'a, LazyTypeNode<'a>> {
        todo!("Phase 1.1b: visitor index 0 (NodeList)")
    }
}

define_lazy_type_node!(LazyTypeSymbol, Kind::TypeSymbol, "Lazy view of `TypeSymbol` (Kind 0x9F).");
impl<'a> LazyTypeSymbol<'a> {
    /// Whether the call has a single element argument.
    pub fn has_element(&self) -> bool { todo!("Phase 1.1b: Common Data bit0") }
    /// `Symbol(...)` callee text.
    pub fn value(&self) -> &'a str { todo!("Phase 1.1b: Extended Data byte 0-1") }
    /// Element argument when present.
    pub fn element(&self) -> Option<LazyTypeNode<'a>> { todo!("Phase 1.1b: visitor index 0") }
}

// ===========================================================================
// Others — pure leaves (5 kinds, no payload, no Common Data)
// ===========================================================================
define_lazy_type_node!(LazyTypeNull, Kind::TypeNull, "Lazy view of `TypeNull` leaf (Kind 0x83).");
define_lazy_type_node!(
    LazyTypeUndefined,
    Kind::TypeUndefined,
    "Lazy view of `TypeUndefined` leaf (Kind 0x84)."
);
define_lazy_type_node!(LazyTypeAny, Kind::TypeAny, "Lazy view of `TypeAny` leaf (Kind 0x85).");
define_lazy_type_node!(LazyTypeUnknown, Kind::TypeUnknown, "Lazy view of `TypeUnknown` leaf (Kind 0x86).");
define_lazy_type_node!(
    LazyTypeUniqueSymbol,
    Kind::TypeUniqueSymbol,
    "Lazy view of `TypeUniqueSymbol` leaf (Kind 0x9E)."
);

// ===========================================================================
// Sum type for any TypeNode variant
// ===========================================================================

/// Wrapper enum produced when the parent node only knows it has *some*
/// TypeNode child (e.g. [`super::comment_ast::LazyJsdocTag::parsed_type`]).
///
/// The decoder reads the child Kind once and constructs the matching
/// variant; downstream callers `match` on it to access per-Kind getters.
#[derive(Debug, Clone, Copy)]
#[allow(missing_docs)]
pub enum LazyTypeNode<'a> {
    // Pattern 1
    Name(LazyTypeName<'a>),
    Number(LazyTypeNumber<'a>),
    StringValue(LazyTypeStringValue<'a>),
    Property(LazyTypeProperty<'a>),
    SpecialNamePath(LazyTypeSpecialNamePath<'a>),
    // Pattern 2
    Union(LazyTypeUnion<'a>),
    Intersection(LazyTypeIntersection<'a>),
    Generic(LazyTypeGeneric<'a>),
    Function(LazyTypeFunction<'a>),
    Object(LazyTypeObject<'a>),
    Tuple(LazyTypeTuple<'a>),
    Parenthesis(LazyTypeParenthesis<'a>),
    NamePath(LazyTypeNamePath<'a>),
    Nullable(LazyTypeNullable<'a>),
    NotNullable(LazyTypeNotNullable<'a>),
    Optional(LazyTypeOptional<'a>),
    Variadic(LazyTypeVariadic<'a>),
    Conditional(LazyTypeConditional<'a>),
    Infer(LazyTypeInfer<'a>),
    KeyOf(LazyTypeKeyOf<'a>),
    TypeOf(LazyTypeTypeOf<'a>),
    Import(LazyTypeImport<'a>),
    Predicate(LazyTypePredicate<'a>),
    Asserts(LazyTypeAsserts<'a>),
    AssertsPlain(LazyTypeAssertsPlain<'a>),
    ReadonlyArray(LazyTypeReadonlyArray<'a>),
    ObjectField(LazyTypeObjectField<'a>),
    JsdocObjectField(LazyTypeJsdocObjectField<'a>),
    IndexedAccessIndex(LazyTypeIndexedAccessIndex<'a>),
    CallSignature(LazyTypeCallSignature<'a>),
    ConstructorSignature(LazyTypeConstructorSignature<'a>),
    TypeParameter(LazyTypeTypeParameter<'a>),
    ParameterList(LazyTypeParameterList<'a>),
    ReadonlyProperty(LazyTypeReadonlyProperty<'a>),
    // Pattern 3
    KeyValue(LazyTypeKeyValue<'a>),
    IndexSignature(LazyTypeIndexSignature<'a>),
    MappedType(LazyTypeMappedType<'a>),
    MethodSignature(LazyTypeMethodSignature<'a>),
    TemplateLiteral(LazyTypeTemplateLiteral<'a>),
    Symbol(LazyTypeSymbol<'a>),
    // Others
    Null(LazyTypeNull<'a>),
    Undefined(LazyTypeUndefined<'a>),
    Any(LazyTypeAny<'a>),
    Unknown(LazyTypeUnknown<'a>),
    UniqueSymbol(LazyTypeUniqueSymbol<'a>),
}

impl<'a> LazyTypeNode<'a> {
    /// Construct the appropriate variant by reading the node's Kind byte.
    ///
    /// Returns `None` when the byte does not match any TypeNode Kind. The
    /// real implementation lands in Phase 1.1b; today this is a `todo!()`
    /// stub since the helper depends on `read_u32`/`from_index` plumbing.
    pub fn from_index(_source_file: &'a LazySourceFile<'a>, _node_index: u32) -> Option<Self> {
        todo!("Phase 1.1b: read Kind byte, dispatch to LazyType*::from_index")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use core::mem::size_of;

    /// Per-Kind lazy structs must fit in 16 bytes.
    #[test]
    fn type_node_lazy_structs_fit_in_16_bytes() {
        macro_rules! assert_size {
            ($t:ty) => {
                assert!(
                    size_of::<$t>() <= 16,
                    concat!(stringify!($t), " exceeds 16 bytes; lazy nodes must stay register-friendly")
                );
            };
        }
        // Spot-check one struct from each pattern group.
        assert_size!(LazyTypeName<'static>); // Pattern 1
        assert_size!(LazyTypeFunction<'static>); // Pattern 2
        assert_size!(LazyTypeKeyValue<'static>); // Pattern 3
        assert_size!(LazyTypeNull<'static>); // Others (leaf)
        assert_size!(LazyTypeMethodSignature<'static>);
        assert_size!(LazyTypeTemplateLiteral<'static>);
    }

    /// The sum-type wrapper carries a discriminant so it is wider, but it
    /// must still fit in registers — 24 bytes is well within budget.
    #[test]
    fn lazy_type_node_sum_fits_in_24_bytes() {
        assert!(size_of::<LazyTypeNode<'static>>() <= 24);
    }
}
