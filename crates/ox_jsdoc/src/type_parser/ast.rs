// @author kazuya kawaguchi (a.k.a. kazupon)
// @license MIT
//

use oxc_allocator::{Box as ArenaBox, Vec as ArenaVec};
use oxc_span::Span;

// ============================================================================
// Common enums
// ============================================================================

/// Position of a prefix/suffix modifier (`?T` vs `T?`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModifierPosition {
    Prefix,
    Suffix,
}

/// Brackets used for generic types (`Array<T>` vs `T[]`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GenericBrackets {
    /// `<T>` angle brackets
    Angle,
    /// `T[]` square brackets
    Square,
}

/// Quote style for string literals and property keys.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QuoteStyle {
    Single,
    Double,
}

/// Separator used in object types.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ObjectSeparator {
    Comma,
    Semicolon,
    Linebreak,
    CommaAndLinebreak,
    SemicolonAndLinebreak,
}

/// Name path type for `A.B`, `A#B`, `A~B`, `A["key"]`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NamePathType {
    /// `.` property access
    Property,
    /// `#` instance member
    Instance,
    /// `~` inner member
    Inner,
    /// `["key"]` bracket access
    PropertyBrackets,
}

/// Special name path prefix (`module:x`, `event:x`, `external:x`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpecialPathType {
    Module,
    Event,
    External,
}

/// Variadic position (`...T` vs `T...` vs bare `...`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VariadicPosition {
    Prefix,
    Suffix,
}

// ============================================================================
// TypeNode enum — 35+ variants
// ============================================================================

/// A parsed JSDoc type expression AST node.
///
/// Each variant corresponds to a jsdoc-type-pratt-parser AST node type.
/// All child nodes use `ArenaBox` for arena allocation.
#[derive(Debug)]
pub enum TypeNode<'a> {
    // --- Basic types ---
    /// Type name: `string`, `number`, `MyClass`
    Name(TypeName<'a>),
    /// Number literal: `42`, `3.14`
    Number(TypeNumber<'a>),
    /// String value: `"hello"`, `'world'`
    StringValue(TypeStringValue<'a>),
    /// `null`
    Null(TypeNull),
    /// `undefined`
    Undefined(TypeUndefined),
    /// Any type: `*`
    Any(TypeAny),
    /// Unknown type: `?` (standalone)
    Unknown(TypeUnknown),

    // --- Compound types ---
    /// Union: `A | B | C`
    Union(TypeUnion<'a>),
    /// Intersection: `A & B & C`
    Intersection(TypeIntersection<'a>),
    /// Generic: `Array<T>`, `T[]`, `Array.<T>`
    Generic(TypeGeneric<'a>),
    /// Function: `function(a): b`, `(a: T) => U`
    Function(TypeFunction<'a>),
    /// Object: `{key: Type}`
    Object(TypeObject<'a>),
    /// Tuple: `[A, B, C]`
    Tuple(TypeTuple<'a>),
    /// Parenthesized: `(T)`
    Parenthesis(TypeParenthesis<'a>),

    // --- Name paths ---
    /// Name path: `A.B`, `A#B`, `A~B`, `A["key"]`
    NamePath(TypeNamePath<'a>),
    /// Special name path: `module:x`, `event:x`, `external:x`
    SpecialNamePath(TypeSpecialNamePath<'a>),

    // --- Modifiers ---
    /// Nullable: `?T`, `T?`
    Nullable(TypeNullable<'a>),
    /// Not nullable: `!T`
    NotNullable(TypeNotNullable<'a>),
    /// Optional: `T=`, `=T`
    Optional(TypeOptional<'a>),
    /// Variadic: `...T`, `T...`, `...[T]`
    Variadic(TypeVariadic<'a>),

    // --- TypeScript-specific ---
    /// Conditional: `A extends B ? C : D`
    Conditional(TypeConditional<'a>),
    /// Infer: `infer T`
    Infer(TypeInfer<'a>),
    /// keyof: `keyof T`
    KeyOf(TypeKeyOf<'a>),
    /// typeof: `typeof X`
    TypeOf(TypeTypeOf<'a>),
    /// import: `import('module')`
    Import(TypeImport<'a>),
    /// Predicate: `x is T`
    Predicate(TypePredicate<'a>),
    /// Asserts: `asserts x is T`
    Asserts(TypeAsserts<'a>),
    /// Asserts plain: `asserts x`
    AssertsPlain(TypeAssertsPlain<'a>),
    /// Readonly array: `readonly T[]`
    ReadonlyArray(TypeReadonlyArray<'a>),
    /// Template literal: `` `text${T}` ``
    TemplateLiteral(TypeTemplateLiteral<'a>),
    /// Unique symbol: `unique symbol`
    UniqueSymbol(TypeUniqueSymbol),

    // --- JSDoc/Closure-specific ---
    /// Symbol: `Symbol(x)`, `MyClass(2)`
    Symbol(TypeSymbol<'a>),

    // --- Non-root / supplementary nodes ---
    /// Object field: `key: Type`
    ObjectField(TypeObjectField<'a>),
    /// JSDoc object field: type as key
    JsdocObjectField(TypeJsdocObjectField<'a>),
    /// Key-value pair (function params): `name: Type`
    KeyValue(TypeKeyValue<'a>),
    /// Property in name path: `prop`
    Property(TypeProperty<'a>),
    /// Index signature: `[key: string]: value`
    IndexSignature(TypeIndexSignature<'a>),
    /// Mapped type: `[K in keyof T]: V`
    MappedType(TypeMappedType<'a>),
    /// Type parameter: `T extends U = V`
    TypeParameter(TypeTypeParameter<'a>),
    /// Call signature: `<T>(...): ReturnType`
    CallSignature(TypeCallSignature<'a>),
    /// Constructor signature: `new <T>(...): Type`
    ConstructorSignature(TypeConstructorSignature<'a>),
    /// Method signature: `method<T>(...): ReturnType`
    MethodSignature(TypeMethodSignature<'a>),
    /// Indexed access index: `T[K]` index part
    IndexedAccessIndex(TypeIndexedAccessIndex<'a>),

    // --- Intermediate nodes (parser internal, not in final AST) ---
    /// Parameter list (intermediate): comma-separated params
    ParameterList(TypeParameterList<'a>),
    /// Readonly property (intermediate): `readonly` keyword before field
    ReadonlyProperty(TypeReadonlyProperty<'a>),
}

impl<'a> TypeNode<'a> {
    #[inline]
    pub fn span(&self) -> Span {
        match self {
            Self::Name(n) => n.span,
            Self::Number(n) => n.span,
            Self::StringValue(n) => n.span,
            Self::Null(n) => n.span,
            Self::Undefined(n) => n.span,
            Self::Any(n) => n.span,
            Self::Unknown(n) => n.span,
            Self::Union(n) => n.span,
            Self::Intersection(n) => n.span,
            Self::Generic(n) => n.span,
            Self::Function(n) => n.span,
            Self::Object(n) => n.span,
            Self::Tuple(n) => n.span,
            Self::Parenthesis(n) => n.span,
            Self::NamePath(n) => n.span,
            Self::SpecialNamePath(n) => n.span,
            Self::Nullable(n) => n.span,
            Self::NotNullable(n) => n.span,
            Self::Optional(n) => n.span,
            Self::Variadic(n) => n.span,
            Self::Conditional(n) => n.span,
            Self::Infer(n) => n.span,
            Self::KeyOf(n) => n.span,
            Self::TypeOf(n) => n.span,
            Self::Import(n) => n.span,
            Self::Predicate(n) => n.span,
            Self::Asserts(n) => n.span,
            Self::AssertsPlain(n) => n.span,
            Self::ReadonlyArray(n) => n.span,
            Self::TemplateLiteral(n) => n.span,
            Self::UniqueSymbol(n) => n.span,
            Self::Symbol(n) => n.span,
            Self::ObjectField(n) => n.span,
            Self::JsdocObjectField(n) => n.span,
            Self::KeyValue(n) => n.span,
            Self::Property(n) => n.span,
            Self::IndexSignature(n) => n.span,
            Self::MappedType(n) => n.span,
            Self::TypeParameter(n) => n.span,
            Self::CallSignature(n) => n.span,
            Self::ConstructorSignature(n) => n.span,
            Self::MethodSignature(n) => n.span,
            Self::IndexedAccessIndex(n) => n.span,
            Self::ParameterList(n) => n.span,
            Self::ReadonlyProperty(n) => n.span,
        }
    }
}

// ============================================================================
// Basic type structs
// ============================================================================

/// Type name: `string`, `number`, `MyClass`
#[derive(Debug)]
pub struct TypeName<'a> {
    pub span: Span,
    pub value: &'a str,
}

/// Number literal: `42`, `3.14`, `-1e10`
#[derive(Debug)]
pub struct TypeNumber<'a> {
    pub span: Span,
    pub value: &'a str,
}

/// String value: `"hello"`, `'world'`
#[derive(Debug)]
pub struct TypeStringValue<'a> {
    pub span: Span,
    pub value: &'a str,
    pub quote: QuoteStyle,
}

/// `null`
#[derive(Debug)]
pub struct TypeNull {
    pub span: Span,
}

/// `undefined`
#[derive(Debug)]
pub struct TypeUndefined {
    pub span: Span,
}

/// Any type: `*`
#[derive(Debug)]
pub struct TypeAny {
    pub span: Span,
}

/// Unknown type: `?` (standalone)
#[derive(Debug)]
pub struct TypeUnknown {
    pub span: Span,
}

// ============================================================================
// Compound type structs
// ============================================================================

/// Union: `A | B | C`
#[derive(Debug)]
pub struct TypeUnion<'a> {
    pub span: Span,
    pub elements: ArenaVec<'a, ArenaBox<'a, TypeNode<'a>>>,
}

/// Intersection: `A & B & C`
#[derive(Debug)]
pub struct TypeIntersection<'a> {
    pub span: Span,
    pub elements: ArenaVec<'a, ArenaBox<'a, TypeNode<'a>>>,
}

/// Generic: `Array<T>`, `T[]`, `Array.<T>`
#[derive(Debug)]
pub struct TypeGeneric<'a> {
    pub span: Span,
    pub left: ArenaBox<'a, TypeNode<'a>>,
    pub elements: ArenaVec<'a, ArenaBox<'a, TypeNode<'a>>>,
    pub brackets: GenericBrackets,
    pub dot: bool,
}

/// Function: `function(a): b`, `(a: T) => U`
#[derive(Debug)]
pub struct TypeFunction<'a> {
    pub span: Span,
    pub parameters: ArenaVec<'a, ArenaBox<'a, TypeNode<'a>>>,
    pub return_type: Option<ArenaBox<'a, TypeNode<'a>>>,
    pub type_parameters: ArenaVec<'a, ArenaBox<'a, TypeNode<'a>>>,
    pub constructor: bool,
    pub arrow: bool,
    pub parenthesis: bool,
}

/// Object: `{key: Type}`
#[derive(Debug)]
pub struct TypeObject<'a> {
    pub span: Span,
    pub elements: ArenaVec<'a, ArenaBox<'a, TypeNode<'a>>>,
    pub separator: Option<ObjectSeparator>,
}

/// Tuple: `[A, B, C]`
#[derive(Debug)]
pub struct TypeTuple<'a> {
    pub span: Span,
    pub elements: ArenaVec<'a, ArenaBox<'a, TypeNode<'a>>>,
}

/// Parenthesized: `(T)`
#[derive(Debug)]
pub struct TypeParenthesis<'a> {
    pub span: Span,
    pub element: ArenaBox<'a, TypeNode<'a>>,
}

// ============================================================================
// Name path structs
// ============================================================================

/// Name path: `A.B`, `A#B`, `A~B`, `A["key"]`
#[derive(Debug)]
pub struct TypeNamePath<'a> {
    pub span: Span,
    pub left: ArenaBox<'a, TypeNode<'a>>,
    pub right: ArenaBox<'a, TypeNode<'a>>,
    pub path_type: NamePathType,
}

/// Special name path: `module:x`, `event:x`, `external:x`
#[derive(Debug)]
pub struct TypeSpecialNamePath<'a> {
    pub span: Span,
    pub value: &'a str,
    pub special_type: SpecialPathType,
    pub quote: Option<QuoteStyle>,
}

// ============================================================================
// Modifier structs
// ============================================================================

/// Nullable: `?T`, `T?`
#[derive(Debug)]
pub struct TypeNullable<'a> {
    pub span: Span,
    pub element: ArenaBox<'a, TypeNode<'a>>,
    pub position: ModifierPosition,
}

/// Not nullable: `!T`
#[derive(Debug)]
pub struct TypeNotNullable<'a> {
    pub span: Span,
    pub element: ArenaBox<'a, TypeNode<'a>>,
    pub position: ModifierPosition,
}

/// Optional: `T=`, `=T`
#[derive(Debug)]
pub struct TypeOptional<'a> {
    pub span: Span,
    pub element: ArenaBox<'a, TypeNode<'a>>,
    pub position: ModifierPosition,
}

/// Variadic: `...T`, `T...`, `...[T]`
#[derive(Debug)]
pub struct TypeVariadic<'a> {
    pub span: Span,
    pub element: Option<ArenaBox<'a, TypeNode<'a>>>,
    pub position: Option<VariadicPosition>,
    pub square_brackets: bool,
}

// ============================================================================
// TypeScript-specific structs
// ============================================================================

/// Conditional: `A extends B ? C : D`
#[derive(Debug)]
pub struct TypeConditional<'a> {
    pub span: Span,
    pub checks_type: ArenaBox<'a, TypeNode<'a>>,
    pub extends_type: ArenaBox<'a, TypeNode<'a>>,
    pub true_type: ArenaBox<'a, TypeNode<'a>>,
    pub false_type: ArenaBox<'a, TypeNode<'a>>,
}

/// Infer: `infer T`
#[derive(Debug)]
pub struct TypeInfer<'a> {
    pub span: Span,
    pub element: ArenaBox<'a, TypeNode<'a>>,
}

/// keyof: `keyof T`
#[derive(Debug)]
pub struct TypeKeyOf<'a> {
    pub span: Span,
    pub element: ArenaBox<'a, TypeNode<'a>>,
}

/// typeof: `typeof X`
#[derive(Debug)]
pub struct TypeTypeOf<'a> {
    pub span: Span,
    pub element: ArenaBox<'a, TypeNode<'a>>,
}

/// import: `import('module')`
#[derive(Debug)]
pub struct TypeImport<'a> {
    pub span: Span,
    pub element: ArenaBox<'a, TypeNode<'a>>,
}

/// Predicate: `x is T`
#[derive(Debug)]
pub struct TypePredicate<'a> {
    pub span: Span,
    pub left: ArenaBox<'a, TypeNode<'a>>,
    pub right: ArenaBox<'a, TypeNode<'a>>,
}

/// Asserts: `asserts x is T`
#[derive(Debug)]
pub struct TypeAsserts<'a> {
    pub span: Span,
    pub left: ArenaBox<'a, TypeNode<'a>>,
    pub right: ArenaBox<'a, TypeNode<'a>>,
}

/// Asserts plain: `asserts x`
#[derive(Debug)]
pub struct TypeAssertsPlain<'a> {
    pub span: Span,
    pub element: ArenaBox<'a, TypeNode<'a>>,
}

/// Readonly array: `readonly T[]`
#[derive(Debug)]
pub struct TypeReadonlyArray<'a> {
    pub span: Span,
    pub element: ArenaBox<'a, TypeNode<'a>>,
}

/// Template literal: `` `text${T}` ``
#[derive(Debug)]
pub struct TypeTemplateLiteral<'a> {
    pub span: Span,
    pub literals: ArenaVec<'a, &'a str>,
    pub interpolations: ArenaVec<'a, ArenaBox<'a, TypeNode<'a>>>,
}

/// Unique symbol: `unique symbol`
#[derive(Debug)]
pub struct TypeUniqueSymbol {
    pub span: Span,
}

// ============================================================================
// JSDoc/Closure-specific structs
// ============================================================================

/// Symbol: `Symbol(x)`, `MyClass(2)`
#[derive(Debug)]
pub struct TypeSymbol<'a> {
    pub span: Span,
    pub value: &'a str,
    pub element: Option<ArenaBox<'a, TypeNode<'a>>>,
}

// ============================================================================
// Non-root / supplementary node structs
// ============================================================================

/// Object field: `key: Type`, `key?: Type`, `readonly key: Type`
#[derive(Debug)]
pub struct TypeObjectField<'a> {
    pub span: Span,
    pub key: ArenaBox<'a, TypeNode<'a>>,
    pub right: Option<ArenaBox<'a, TypeNode<'a>>>,
    pub optional: bool,
    pub readonly: bool,
    pub quote: Option<QuoteStyle>,
}

/// JSDoc object field: type as key (jsdoc mode, `allowKeyTypes: true`)
#[derive(Debug)]
pub struct TypeJsdocObjectField<'a> {
    pub span: Span,
    pub left: ArenaBox<'a, TypeNode<'a>>,
    pub right: ArenaBox<'a, TypeNode<'a>>,
}

/// Key-value pair (function params): `name: Type`
#[derive(Debug)]
pub struct TypeKeyValue<'a> {
    pub span: Span,
    pub key: &'a str,
    pub right: Option<ArenaBox<'a, TypeNode<'a>>>,
    pub optional: bool,
    pub variadic: bool,
}

/// Property in name path: `prop`, `"prop"`
#[derive(Debug)]
pub struct TypeProperty<'a> {
    pub span: Span,
    pub value: &'a str,
    pub quote: Option<QuoteStyle>,
}

/// Index signature: `[key: string]: value`
#[derive(Debug)]
pub struct TypeIndexSignature<'a> {
    pub span: Span,
    pub key: &'a str,
    pub right: ArenaBox<'a, TypeNode<'a>>,
}

/// Mapped type: `[K in keyof T]: V`
#[derive(Debug)]
pub struct TypeMappedType<'a> {
    pub span: Span,
    pub key: &'a str,
    pub right: ArenaBox<'a, TypeNode<'a>>,
}

/// Type parameter: `T extends U = V`
#[derive(Debug)]
pub struct TypeTypeParameter<'a> {
    pub span: Span,
    pub name: ArenaBox<'a, TypeNode<'a>>,
    pub constraint: Option<ArenaBox<'a, TypeNode<'a>>>,
    pub default_value: Option<ArenaBox<'a, TypeNode<'a>>>,
}

/// Call signature: `<T>(...): ReturnType`
#[derive(Debug)]
pub struct TypeCallSignature<'a> {
    pub span: Span,
    pub parameters: ArenaVec<'a, ArenaBox<'a, TypeNode<'a>>>,
    pub return_type: ArenaBox<'a, TypeNode<'a>>,
    pub type_parameters: ArenaVec<'a, ArenaBox<'a, TypeNode<'a>>>,
}

/// Constructor signature: `new <T>(...): Type`
#[derive(Debug)]
pub struct TypeConstructorSignature<'a> {
    pub span: Span,
    pub parameters: ArenaVec<'a, ArenaBox<'a, TypeNode<'a>>>,
    pub return_type: ArenaBox<'a, TypeNode<'a>>,
    pub type_parameters: ArenaVec<'a, ArenaBox<'a, TypeNode<'a>>>,
}

/// Method signature: `method<T>(...): ReturnType`
#[derive(Debug)]
pub struct TypeMethodSignature<'a> {
    pub span: Span,
    pub name: &'a str,
    pub parameters: ArenaVec<'a, ArenaBox<'a, TypeNode<'a>>>,
    pub return_type: ArenaBox<'a, TypeNode<'a>>,
    pub type_parameters: ArenaVec<'a, ArenaBox<'a, TypeNode<'a>>>,
    pub quote: Option<QuoteStyle>,
}

/// Indexed access index: `T[K]` index part
#[derive(Debug)]
pub struct TypeIndexedAccessIndex<'a> {
    pub span: Span,
    pub right: ArenaBox<'a, TypeNode<'a>>,
}

// ============================================================================
// Intermediate nodes (parser internal)
// ============================================================================

/// Parameter list (intermediate): comma-separated params.
/// Converted to `TypeFunction.parameters` by the parser.
#[derive(Debug)]
pub struct TypeParameterList<'a> {
    pub span: Span,
    pub elements: ArenaVec<'a, ArenaBox<'a, TypeNode<'a>>>,
}

/// Readonly property (intermediate): `readonly` keyword before field.
/// Converted to `TypeObjectField.readonly = true` by the parser.
#[derive(Debug)]
pub struct TypeReadonlyProperty<'a> {
    pub span: Span,
    pub element: ArenaBox<'a, TypeNode<'a>>,
}

// ============================================================================
// Parse mode
// ============================================================================

/// Parse mode for the type parser.
///
/// Matches jsdoc-type-pratt-parser's 3 modes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParseMode {
    /// JSDoc mode: supports all JSDoc-specific syntax (loose lexer).
    Jsdoc,
    /// Closure mode: supports Closure Compiler syntax (loose lexer).
    Closure,
    /// TypeScript mode: supports TypeScript-specific syntax (strict lexer).
    Typescript,
}

impl ParseMode {
    /// Returns `true` if the lexer should use loose rules (NaN, Infinity, hyphens).
    #[inline]
    pub fn is_loose(self) -> bool {
        matches!(self, Self::Jsdoc | Self::Closure)
    }

    /// Returns `true` if this is jsdoc mode.
    #[inline]
    pub fn is_jsdoc(self) -> bool {
        self == Self::Jsdoc
    }

    /// Returns `true` if this is closure mode.
    #[inline]
    pub fn is_closure(self) -> bool {
        self == Self::Closure
    }

    /// Returns `true` if this is typescript mode.
    #[inline]
    pub fn is_typescript(self) -> bool {
        self == Self::Typescript
    }
}
