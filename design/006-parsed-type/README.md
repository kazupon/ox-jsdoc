# parsedType — JSDoc Type Expression Parser

## Background

ox-jsdoc currently stores JSDoc type expressions (e.g. `{string | number}`, `{Array<T>}`)
as raw text in `rawType`. A `parsedType` field provides a structured AST for type
expressions, enabling type validation, transformation, and fixer support.

## No Official Specification

JSDoc type expression syntax has **no formal specification** (unlike ECMAScript/TC39).
The syntax is defined by implementations:

| Source                     | Role                            | URL                                                                              |
| -------------------------- | ------------------------------- | -------------------------------------------------------------------------------- |
| JSDoc docs                 | Tool user guide (not a spec)    | https://jsdoc.app/tags-type                                                      |
| Google Closure Compiler    | Closure type system definition  | https://github.com/google/closure-compiler/wiki/Types-in-the-Closure-Type-System |
| TypeScript JSDoc Reference | TS-recognized JSDoc types       | https://www.typescriptlang.org/docs/handbook/jsdoc-supported-types.html          |
| jsdoc-type-pratt-parser    | De facto unified spec (3 modes) | https://github.com/jsdoc-type-pratt-parser/jsdoc-type-pratt-parser               |

For the eslint-plugin-jsdoc ecosystem, **jsdoc-type-pratt-parser's implementation is
the de facto specification**.

## Reference Implementations

### jsdoc-type-pratt-parser (v7.2.0)

- Language: TypeScript
- Algorithm: Pratt parser (precedence climbing)
- 3 parse modes: `jsdoc`, `closure`, `typescript`
- 34+ AST node types
- Used by: `@es-joy/jsdoccomment` → `eslint-plugin-jsdoc`
- Source: `refers/jsdoc-type-pratt-parser/`

### typescript-go

- Language: Go
- Algorithm: Recursive descent (reuses TypeScript type parser)
- 1 mode: TypeScript + 5 JSDoc wrapper nodes
- Source: `refers/typescript-go/internal/parser/jsdoc.go`

### oxc_parser

- Language: Rust
- Algorithm: Recursive descent (TypeScript type parser)
- 3 JSDoc type nodes only: `JSDocNullableType`, `JSDocNonNullableType`, `JSDocUnknownType`
- `JSDocAllType` and `JSDocFunctionType` are TODO
- Source: `refers/oxc/crates/oxc_parser/src/ts/types.rs`

## Syntax Comparison: jsdoc-type-pratt-parser vs typescript-go

### Both Support (common ground)

| Syntax           | Example                       | Notes                |
| ---------------- | ----------------------------- | -------------------- |
| Basic types      | `string`, `number`, `boolean` |                      |
| Nullable prefix  | `?string`                     | JSDocNullableType    |
| Nullable suffix  | `string?`                     | JSDocNullableType    |
| Non-nullable     | `!string`                     | JSDocNonNullableType |
| Any wildcard     | `*`                           | JSDocAllType         |
| Unknown          | `?` (standalone)              | JSDocUnknownType     |
| Optional suffix  | `string=`                     | JSDocOptionalType    |
| Variadic prefix  | `...string`                   | JSDocVariadicType    |
| Union            | `string \| number`            |                      |
| Intersection     | `A & B`                       |                      |
| Generic          | `Array<string>`, `Map<K,V>`   |                      |
| Array shorthand  | `string[]`                    |                      |
| Tuple            | `[string, number]`            |                      |
| Arrow function   | `(a: string) => number`       |                      |
| Conditional      | `T extends U ? X : Y`         |                      |
| keyof            | `keyof MyType`                |                      |
| typeof           | `typeof myVar`                |                      |
| import           | `import('./module')`          |                      |
| Template literal | `` `${string}-${number}` ``   |                      |
| readonly array   | `readonly string[]`           |                      |
| Predicate        | `x is string`                 |                      |
| Asserts          | `asserts x is T`              |                      |
| Parenthesized    | `(string \| number)`          |                      |
| Name path dot    | `Foo.Bar`                     |                      |
| String literal   | `"hello"`                     |                      |
| Number literal   | `42`, `3.14`                  |                      |
| null / undefined | `null`, `undefined`           |                      |

### jsdoc-type-pratt-parser Only (NOT in typescript-go)

#### 1. Closure Function Syntax — Impact: HIGH

```javascript
/**
 * @type {function(string, boolean): number}
 * @callback {function(Error, Response): void}
 * @param {function(this:MyObj, string): boolean} handler
 */
```

- jsdoc-type-pratt-parser: Supported (jsdoc/closure modes)
- typescript-go: NOT supported (only `(a: string) => number` arrow syntax)
- Prevalence: Common in Closure Compiler projects and older JSDoc

#### 2. Dot-Notation Generics — Impact: MEDIUM

```javascript
/**
 * @type {Array.<string>}
 * @type {Object.<string, number>}
 * @param {Map.<string, Array.<number>>} data
 */
```

- jsdoc-type-pratt-parser: Supported (all modes)
- typescript-go: Parses but reports as grammar error
- Prevalence: Common in older JSDoc, still seen in legacy codebases

#### 3. Module Name Path — Impact: MEDIUM

```javascript
/**
 * @type {module:app/services/UserService}
 * @see {module:utils/helpers.formatDate}
 * @param {module:models.User} user
 */
```

- jsdoc-type-pratt-parser: Supported (jsdoc: module/event/external, closure: module only)
- typescript-go: NOT supported
- Prevalence: Common in Node.js projects using JSDoc modules

#### 4. Event/External Name Path — Impact: LOW

```javascript
/**
 * @fires {event:user-login}
 * @fires {event:'data-received'}
 * @see {external:jQuery}
 * @see {external:XMLHttpRequest}
 */
```

- jsdoc-type-pratt-parser: Supported (jsdoc mode only)
- typescript-go: NOT supported
- Prevalence: Rare, primarily in event-driven documentation

#### 5. Symbol Syntax — Impact: LOW

```javascript
/**
 * @type {Symbol(iterator)}
 * @type {MyClass(2)}
 */
```

- jsdoc-type-pratt-parser: Supported (jsdoc/closure modes)
- typescript-go: NOT supported (only `symbol` keyword)
- Prevalence: Rare

#### 6. Optional Prefix — Impact: LOW

```javascript
/**
 * @param {=string} name
 */
```

- jsdoc-type-pratt-parser: Supported
- typescript-go: NOT supported (only postfix `Type=`)
- Prevalence: Very rare, `Type=` (postfix) is the standard form

#### 7. Variadic Postfix — Impact: LOW

```javascript
/**
 * @param {string...} names
 */
```

- jsdoc-type-pratt-parser: Supported (jsdoc mode only)
- typescript-go: NOT supported (only prefix `...Type`)
- Prevalence: Rare, `...Type` (prefix) is the standard form

#### 8. Variadic with Enclosing Brackets — Impact: LOW

```javascript
/**
 * @param {...[string]} names
 */
```

- jsdoc-type-pratt-parser: Supported (jsdoc mode only)
- typescript-go: NOT supported
- Prevalence: Rare

#### 9. Bare `function` Type — Impact: LOW

```javascript
/**
 * @type {function}
 * @param {function} callback
 */
```

- jsdoc-type-pratt-parser: Supported (jsdoc mode, no parentheses required)
- typescript-go: NOT supported (requires `function()`)
- Prevalence: Occasional in loose JSDoc

#### 10. Name Path Instance/Inner — Impact: LOW

```javascript
/**
 * @type {MyClass#instanceMethod}
 * @type {MyModule~innerFunction}
 */
```

- jsdoc-type-pratt-parser: Supported (`#` instance, `~` inner)
- typescript-go: NOT supported (only `.` dot paths)
- Prevalence: Occasional in JSDoc documentation

#### 11. Full Types as Object Keys — Impact: LOW

```javascript
/**
 * @type {{string: number, Array<string>: boolean}}
 */
```

- jsdoc-type-pratt-parser: Supported (jsdoc mode, `allowKeyTypes: true`)
- typescript-go: NOT supported
- Prevalence: Very rare

## jsdoc-type-pratt-parser AST Node Types (34+)

### Root Nodes (RootResult union)

```
JsdocTypeName             - string, number, MyClass
JsdocTypeNumber           - 42, 3.14
JsdocTypeStringValue      - "hello"
JsdocTypeNull             - null
JsdocTypeUndefined        - undefined
JsdocTypeAny              - *
JsdocTypeUnknown          - ?
JsdocTypeUnion            - A | B | C
JsdocTypeIntersection     - A & B & C              (typescript only)
JsdocTypeGeneric          - Array<T>, T[]
JsdocTypeFunction         - function(a): b
JsdocTypeObject           - {key: Type}
JsdocTypeTuple            - [A, B, C]               (typescript only)
JsdocTypeNamePath         - A.B, A#B, A~B, A["key"]
JsdocTypeSpecialNamePath  - module:x, event:x
JsdocTypeSymbol           - Symbol(0)                (jsdoc/closure only)
JsdocTypeUniqueSymbol     - unique symbol             (typescript only)
JsdocTypeTypeof           - typeof X
JsdocTypeKeyof            - keyof T                  (typescript only)
JsdocTypeImport           - import('module')          (typescript only)
JsdocTypeInfer            - infer T                  (typescript only)
JsdocTypeOptional         - T=, =T
JsdocTypeNullable         - ?T, T?
JsdocTypeNotNullable      - !T
JsdocTypeVariadic         - ...T, T...
JsdocTypeParenthesis      - (T)
JsdocTypeConditional      - A extends B ? C : D     (typescript only)
JsdocTypePredicate        - x is T                  (typescript only)
JsdocTypeAsserts          - asserts x is T           (typescript only)
JsdocTypeAssertsPlain     - asserts x                (typescript only)
JsdocTypeReadonlyArray    - readonly T[]             (typescript only)
JsdocTypeTemplateLiteral  - `text${T}`               (typescript only)
```

### Non-Root Nodes (supplementary)

```
JsdocTypeObjectField            - Object property field
JsdocTypeJsdocObjectField       - JSDoc-style object field (type as key)
JsdocTypeKeyValue               - key: value pair (function params)
JsdocTypeProperty               - Single property in name path
JsdocTypeIndexSignature         - [key: string]: value
JsdocTypeMappedType             - [K in keyof T]: V
JsdocTypeTypeParameter          - T extends U = V
JsdocTypeCallSignature          - <T>(...): ReturnType
JsdocTypeConstructorSignature   - new <T>(...): Type
JsdocTypeMethodSignature        - method<T>(...): ReturnType
JsdocTypeComputedProperty       - [expr]: Type
JsdocTypeComputedMethod         - [expr]<T>(...): Type
JsdocTypeIndexedAccessIndex     - T[K] index part
```

## Design Options Considered

### Option A: typescript-go Approach

Reuse oxc_parser's TypeScript type parser + add 5 JSDoc wrapper nodes.

- Pros: Minimal new code, leverages existing well-tested parser
- Cons: Missing Closure function syntax, dot-notation generics, special name paths
- Coverage: ~90% of real-world JSDoc type usage

### Option B: jsdoc-type-pratt-parser Compatible (CHOSEN)

Implement a full Pratt parser in Rust matching jsdoc-type-pratt-parser's behavior.

- Pros: 100% eslint-plugin-jsdoc compatibility, all 3 modes
- Cons: Significant implementation effort (~34 AST node types)
- Coverage: 100%

### Option C: Hybrid

Use oxc_parser for TypeScript type syntax + implement JSDoc-specific extensions
as a thin layer.

- Pros: Best of both worlds, incremental approach
- Cons: Requires careful integration between two parsers
- Coverage: ~98%

### Option D: Delegate to JS (Strategy B)

Keep `parsedType: null` in ox-jsdoc. Let jsdoccomment call jsdoc-type-pratt-parser
on the JS side using ox-jsdoc's `rawType` output.

- Pros: Zero implementation effort, guaranteed compatibility
- Cons: No Rust-side type AST, type parsing cost stays in JS
- Coverage: 100% (delegated)

### Decision: Option B

eslint-plugin-jsdoc requires full jsdoc-type-pratt-parser compatibility when
ox-jsdoc replaces comment-parser in jsdoccomment. Options A and C leave gaps
in Closure/JSDoc syntax. Option D keeps type parsing cost in JS.
Option B provides 100% coverage with maximum performance via Rust Pratt parser.

## Practical Impact Assessment

For TypeScript projects (most common modern usage):

- typescript-go approach covers **100%** of needed syntax
- Closure-specific syntax (#1, #2) is irrelevant

For legacy JSDoc / Closure Compiler projects:

- Closure function syntax (#1) and dot-notation generics (#2) are **frequently used**
- jsdoc-type-pratt-parser compatibility is important

For eslint-plugin-jsdoc integration:

- Full compatibility is required for drop-in replacement
- All 3 parse modes must produce identical AST output
- However, the simplest path for eslint-plugin-jsdoc is Option D (call
  jsdoc-type-pratt-parser on the JS side), where jsdoccomment generates
  parsedType after ox-jsdoc provides rawType

## Strategy

### jsdoc-type-pratt-parser compatible Pratt parser in Rust

To enable full operation when replacing comment-parser with ox-jsdoc in jsdoccomment
for eslint-plugin-jsdoc, all 3 parse modes of jsdoc-type-pratt-parser are supported.
Performance is prioritized using a Pratt parser approach with oxc-style optimizations.

## Architectural Consistency with Comment Parser

ox-jsdoc is a high-performance project. The type parser follows the same
architectural patterns as the existing comment parser to ensure consistency,
maintainability, and shared infrastructure.

### Shared Infrastructure

The type parser reuses the same foundations as the comment parser:

| Infrastructure | Comment Parser                             | Type Parser                                   | Shared?               |
| -------------- | ------------------------------------------ | --------------------------------------------- | --------------------- |
| Allocator      | `&'a Allocator`                            | `&'a Allocator`                               | Same instance         |
| AST arena      | `ArenaBox`, `ArenaVec`                     | `ArenaBox`, `ArenaVec`                        | Same allocator        |
| Span           | `oxc_span::Span` (absolute offsets)        | `oxc_span::Span` (absolute offsets)           | Same type             |
| Error type     | `OxcDiagnostic` via `ParserDiagnosticKind` | `OxcDiagnostic` via `TypeDiagnosticKind`      | Same framework        |
| Checkpoint     | `parser::Checkpoint`                       | `Lexer::save()/restore()` (separate approach) | Managed independently |
| Zero-copy      | `&'a str` slices from source               | `&'a str` slices from source                  | Same pattern          |
| base_offset    | `ParserContext.base_offset`                | Passed to Lexer                               | Same pattern          |

### Unified Error Handling

Both parsers use `OxcDiagnostic` through the same `diagnostic()` helper pattern.
`TypeDiagnosticKind` enum and `type_diagnostic()` function are added to the
existing `parser/diagnostics.rs` (not a separate file).

```rust
// parser/diagnostics.rs — consolidated in one file

// Existing (unchanged)
pub enum ParserDiagnosticKind {
    NotAJSDocBlock,
    UnclosedBlockComment,
    SpanOverflow,
    UnclosedInlineTag,
    UnclosedTypeExpression,
    UnclosedFence,
    InvalidTagStart,
    InvalidInlineTagStart,
}

pub fn diagnostic(kind: ParserDiagnosticKind) -> OxcDiagnostic {
    // ... existing implementation
}

// Added: for type parser
pub enum TypeDiagnosticKind {
    NoParsletFound,
    ExpectedToken,
    UnclosedGeneric,
    UnclosedParenthesis,
    InvalidTypeExpression,
}

pub fn type_diagnostic(kind: TypeDiagnosticKind) -> OxcDiagnostic {
    let message = match kind {
        TypeDiagnosticKind::NoParsletFound => "unexpected token in type expression",
        TypeDiagnosticKind::ExpectedToken => "expected token in type expression",
        TypeDiagnosticKind::UnclosedGeneric => "generic type expression is not closed",
        TypeDiagnosticKind::UnclosedParenthesis => "parenthesized type is not closed",
        TypeDiagnosticKind::InvalidTypeExpression => "invalid type expression",
    };
    OxcDiagnostic::error(message)
}
```

No separate `TypeError` type. All errors flow through `OxcDiagnostic`.

This consolidation means:

- Both parsers accumulate diagnostics in the same `Vec<OxcDiagnostic>`
- Type parser errors can be pushed directly to `ParserContext.diagnostics` without conversion
- Error conversion functions live in the same module, keeping dependencies simple

### Diagnostics Propagation

Type parsing is a method on ParserContext, so `self.diagnostics` is used directly.
No argument passing or type conversion needed:

```rust
// Type parse method on ParserContext
fn parse_type_pratt(
    &mut self,
    lexer: &mut Lexer<'a>,
    disallow_conditional: &mut bool,
    min_precedence: Precedence,
) -> Option<TypeNode<'a>> {
    // On error: same pattern as comment parser
    self.diagnostics.push(type_diagnostic(TypeDiagnosticKind::NoParsletFound));
    // ...
}
```

Call from comment parser:

```rust
// In parse_generic_tag_body()
let node = self.parse_type_expression(type_source.raw, base, mode);
parsed_type = match node {
    Some(n) => Some(ArenaBox::new_in(JsdocType::Parsed(n), self.allocator)),
    None => Some(ArenaBox::new_in(JsdocType::Raw(type_source), self.allocator)),
};
```

### Checkpoint Separation

The comment parser and type parser need to save/restore entirely different state,
so separate approaches are used:

- **Comment parser**: Existing `Checkpoint` struct (`parser/checkpoint.rs`) unchanged
- **Type parser**: `Lexer::save()/restore()` implementation. `diagnostics_len` managed by ParserContext

```rust
// type_parser/lexer.rs

/// Lexer state snapshot (32 bytes, Copy).
#[derive(Debug, Clone, Copy)]
pub struct LexerState {
    pub offset: usize,      // 8 bytes
    pub current: Token,      // 12 bytes
    pub next: Token,         // 12 bytes
}

impl<'a> Lexer<'a> {
    #[inline]
    pub fn save(&self) -> LexerState {
        LexerState { offset: self.offset, current: self.current, next: self.next }
    }

    #[inline]
    pub fn restore(&mut self, state: LexerState) {
        self.offset = state.offset;
        self.current = state.current;
        self.next = state.next;
    }
}
```

Speculative parsing on ParserContext side:

```rust
// parser/type_parse.rs
fn try_parse_type<F, T>(
    &mut self,
    lexer: &mut Lexer<'a>,
    f: F,
) -> Option<T>
where
    F: FnOnce(&mut Self, &mut Lexer<'a>) -> Option<T>,
{
    let saved_lexer = lexer.save();
    let saved_diag_len = self.diagnostics.len();
    let result = f(self, lexer);
    if result.is_none() {
        lexer.restore(saved_lexer);
        self.diagnostics.truncate(saved_diag_len);
    }
    result
}
```

All stack-local value copies (32 bytes), no heap allocation.

### Unified Parser Struct

No `TypeParser` struct. Type parsing logic is integrated directly as methods
on `ParserContext`. This eliminates all field duplication for `allocator`,
`source_text`, `base_offset`, and `diagnostics`:

```rust
impl<'a> ParserContext<'a> {
    /// Parse {type} text into a type expression AST.
    /// Lexer is created on the stack. allocator and diagnostics use self.
    fn parse_type_expression(
        &mut self,
        type_text: &'a str,
        type_base_offset: u32,
        mode: ParseMode,
    ) -> Option<ArenaBox<'a, TypeNode<'a>>> {
        let mut lexer = Lexer::new(type_text, type_base_offset, mode.is_loose());
        let mut disallow_conditional = false;
        self.parse_type_pratt(&mut lexer, &mut disallow_conditional, Precedence::All)
    }
}
```

Benefits:

- No `TypeParser` struct — zero field duplication
- `self.allocator`, `self.diagnostics` used directly — no sharing/propagation issues
- Lexer is a stack-local temporary — automatically released on type parse completion
- Only one lifetime `'a` — no complexity
- No standalone `parse_type()` public API needed (no external consumers)

### Call Flow

Type parsing completes entirely within ParserContext:

```
parse_comment() [ParserContext]
  |
  +-- scanner::logical_lines()
  +-- partition_sections()
  +-- parse_description_lines()
  +-- parse_jsdoc_tag()
        +-- parse_generic_tag_body()
              |
              +-- extract rawType "{...}"
              +-- self.parse_type_expression()     <-- same ParserContext
                    |
                    +-- Create Lexer on stack
                    +-- Allocate TypeNode in arena via self.allocator
                    +-- Push errors to self.diagnostics directly
                    +-- Return TypeNode (Option)
```

### Justified Differences

| Difference                                  | Reason                                                                  |
| ------------------------------------------- | ----------------------------------------------------------------------- |
| Comment parser: line-oriented (LogicalLine) | JSDoc comments are structurally line-based (margin, tags at line start) |
| Type parser: token-oriented (Lexer + Token) | Type expressions are inline token streams (operators, nesting)          |
| Comment parser: no Precedence               | No operator precedence in comment structure                             |
| Type parser: Precedence enum + Pratt loop   | Type operators have precedence (union < intersection < generic)         |
| Comment parser: FenceState + QuoteKind      | Fenced code blocks exist in comments                                    |
| Type parser: no fence state                 | No fenced code blocks inside type expressions                           |

These differences are inherent to what each parser processes, not architectural divergence.

## Architecture

The type parser is implemented as a submodule within `crates/ox_jsdoc`.
AST definitions live in `type_parser/ast.rs` and are re-exported from `ast.rs`
for seamless integration with the existing JSDoc AST.

```
crates/
  ox_jsdoc/
    src/
      ast.rs                <-- JsdocType::Parsed(TypeNode), re-exports TypeNode
      parser/
        checkpoint.rs       <-- Checkpoint (comment parser)
        diagnostics.rs      <-- ParserDiagnosticKind + TypeDiagnosticKind (one file)
        context.rs          <-- ParserContext (comment parsing)
        type_parse.rs       <-- impl ParserContext type parse methods (Pratt loop etc.)
        scanner.rs          <-- LogicalLine + MarginInfo (comment scanner)
        mod.rs              <-- parse_comment() entry point
      type_parser/          <-- Type parser support code
        mod.rs              <-- re-export
        ast.rs              <-- TypeNode enum + all type node structs
        lexer.rs            <-- Lexer + Token + LexerState
        precedence.rs       <-- Precedence enum
        stringify.rs        <-- AST to string reconstruction
```

`parser/type_parse.rs` distributes `impl ParserContext` block into a separate file.
It implements methods on the same `ParserContext` as `context.rs`, and the generated
binary is identical to having everything in one file (Rust compilation unit is the crate).

### AST Placement

`TypeNode` is defined in `type_parser/ast.rs` and re-exported from `ast.rs`:

```rust
// ast.rs
pub use crate::type_parser::ast::TypeNode;

pub enum JsdocType<'a> {
    Parsed(ArenaBox<'a, TypeNode<'a>>),
    Raw(JsdocTypeSource<'a>),
}
```

### AST Placement Rationale

`TypeNode` is defined in `type_parser/ast.rs` and re-exported from `ast.rs` because:

- `JsdocTag.parsed_type` can seamlessly reference `TypeNode`
- type_parser module maintains independence (internal changes don't propagate to ast.rs)
- `ast.rs` avoids bloat (30+ TypeNode variants stay in type_parser/ast.rs)

## Binary AST / Existing AST Integration

### 1. Span: Absolute Byte Offsets

All TypeNode Spans use **absolute byte offsets relative to the source file**.
`parse_type_expression()` passes `base_offset` to the Lexer, which adds this
offset to all token positions.

### 2. NodeKind: Unified Numbering

A single `NodeKind` enum (`#[repr(u16)]`) manages both JSDoc comment AST nodes
and TypeNode variants for Binary AST compatibility:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u16)]
pub enum NodeKind {
    // --- JSDoc comment AST (0-19) ---
    JsdocBlock = 0,
    JsdocTag = 1,
    JsdocDescriptionLine = 2,
    JsdocTypeLine = 3,
    JsdocInlineTag = 4,
    // 5-19: reserved

    // --- Type AST: basic types (20-29) ---
    TypeName = 20,
    TypeNumber = 21,
    TypeStringValue = 22,
    TypeNull = 23,
    TypeUndefined = 24,
    TypeAny = 25,
    TypeUnknown = 26,

    // --- Type AST: compound types (30-39) ---
    TypeUnion = 30,
    TypeIntersection = 31,
    TypeGeneric = 32,
    TypeFunction = 33,
    TypeObject = 34,
    TypeTuple = 35,
    TypeParenthesis = 36,

    // --- Type AST: name paths (40-44) ---
    TypeNamePath = 40,
    TypeSpecialNamePath = 41,
    TypeProperty = 42,

    // --- Type AST: modifiers (45-49) ---
    TypeNullable = 45,
    TypeNotNullable = 46,
    TypeOptional = 47,
    TypeVariadic = 48,

    // --- Type AST: TypeScript-specific (50-64) ---
    TypeConditional = 50,
    TypeInfer = 51,
    TypeKeyOf = 52,
    TypeTypeOf = 53,
    TypeImport = 54,
    TypePredicate = 55,
    TypeAsserts = 56,
    TypeAssertsPlain = 57,
    TypeReadonlyArray = 58,
    TypeTemplateLiteral = 59,

    // --- Type AST: JSDoc/Closure-specific (65-69) ---
    TypeSymbol = 65,
    TypeUniqueSymbol = 66,

    // --- Type AST: object/function internals (70-84) ---
    TypeObjectField = 70,
    TypeJsdocObjectField = 71,
    TypeKeyValue = 72,
    TypeIndexSignature = 73,
    TypeMappedType = 74,
    TypeTypeParameter = 75,
    TypeCallSignature = 76,
    TypeConstructorSignature = 77,
    TypeMethodSignature = 78,
    TypeComputedProperty = 79,
    TypeComputedMethod = 80,
    TypeIndexedAccessIndex = 81,
    TypeParameterList = 82,     // intermediate only
}
```

### 3. Meta Fields Mapping

Mapping from jsdoc-type-pratt-parser's meta fields to ox-jsdoc TypeNode struct fields:

| jsdoc-type-pratt-parser meta   | ox-jsdoc TypeNode field                             | Usage                                     |
| ------------------------------ | --------------------------------------------------- | ----------------------------------------- |
| `position: 'prefix'\|'suffix'` | `ModifierPosition` enum                             | Nullable, Optional, Variadic, NotNullable |
| `brackets: 'angle'\|'square'`  | `GenericBrackets` enum                              | Generic `<T>` vs `T[]`                    |
| `dot: boolean`                 | `TypeGeneric.dot: bool`                             | `Array.<T>` dot notation                  |
| `quote: 'single'\|'double'`    | `TypeStringValue.quote: QuoteStyle`                 | String literal quote style                |
| `separator`                    | `TypeObject.separator: ObjectSeparator`             | Object field separator                    |
| `parenthesis: boolean`         | `TypeFunction.parenthesis: bool`                    | function has parentheses                  |
| `arrow: boolean`               | `TypeFunction.arrow: bool`                          | `=>` vs `function()`                      |
| `constructor: boolean`         | `TypeFunction.constructor: bool`                    | `new function()`                          |
| `squareBrackets: boolean`      | `TypeVariadic.square_brackets: bool`                | `...[T]` brackets                         |
| `pathType`                     | `TypeNamePath.path_type: NamePathType`              | `.` `#` `~` `["key"]`                     |
| `specialType`                  | `TypeSpecialNamePath.special_type: SpecialPathType` | `module:x`                                |
| `optional: boolean`            | `TypeObjectField.optional: bool`                    | `{key?: T}`                               |
| `readonly: boolean`            | `TypeObjectField.readonly: bool`                    | `{readonly key: T}`                       |

### 4. Binary AST Flattening

Binary AST is not yet implemented. This section confirms that the TypeNode design
is compatible with future Binary AST flattening.

The Binary AST encoder flattens the recursive TypeNode tree into a linear node array
using DFS order. JSDoc nodes and TypeNodes coexist in the same flat array,
distinguished by NodeKind.

```
// Input: string | Array<number>
// TypeNode AST:
//   TypeUnion
//     +-- TypeName("string")
//     +-- TypeGeneric
//           +-- TypeName("Array")
//           +-- TypeName("number")

// Binary AST flattening (DFS order):
// Node[0]: Kind=TypeUnion,   NodeData=Children(first=1, count=2)
// Node[1]: Kind=TypeName,    NodeData=StringIndex(->"string")
// Node[2]: Kind=TypeGeneric, NodeData=Children(first=3, count=2), Flags=Angle
// Node[3]: Kind=TypeName,    NodeData=StringIndex(->"Array")
// Node[4]: Kind=TypeName,    NodeData=StringIndex(->"number")
```

JSDoc nodes and TypeNodes coexist in the same flat array:

```
// Node[0]: Kind=JsdocBlock, ...
// Node[1]: Kind=JsdocTag,   ...
// Node[2]: Kind=TypeUnion,  ...    <-- tag.parsed_type root node
// Node[3]: Kind=TypeName,   ...
// Node[4]: Kind=TypeName,   ...
```

### 5. JSON type Field Mapping

| TypeNode Variant     | JSON `type`                     |
| -------------------- | ------------------------------- |
| `Name`               | `"JsdocTypeName"`               |
| `Number`             | `"JsdocTypeNumber"`             |
| `StringValue`        | `"JsdocTypeStringValue"`        |
| `Null`               | `"JsdocTypeNull"`               |
| `Undefined`          | `"JsdocTypeUndefined"`          |
| `Any`                | `"JsdocTypeAny"`                |
| `Unknown`            | `"JsdocTypeUnknown"`            |
| `Union`              | `"JsdocTypeUnion"`              |
| `Intersection`       | `"JsdocTypeIntersection"`       |
| `Generic`            | `"JsdocTypeGeneric"`            |
| `Function`           | `"JsdocTypeFunction"`           |
| `Object`             | `"JsdocTypeObject"`             |
| `Tuple`              | `"JsdocTypeTuple"`              |
| `NamePath`           | `"JsdocTypeNamePath"`           |
| `SpecialNamePath`    | `"JsdocTypeSpecialNamePath"`    |
| `Symbol`             | `"JsdocTypeSymbol"`             |
| `UniqueSymbol`       | `"JsdocTypeUniqueSymbol"`       |
| `TypeOf`             | `"JsdocTypeTypeof"`             |
| `KeyOf`              | `"JsdocTypeKeyof"`              |
| `Import`             | `"JsdocTypeImport"`             |
| `Infer`              | `"JsdocTypeInfer"`              |
| `Optional`           | `"JsdocTypeOptional"`           |
| `Nullable`           | `"JsdocTypeNullable"`           |
| `NotNullable`        | `"JsdocTypeNotNullable"`        |
| `Variadic`           | `"JsdocTypeVariadic"`           |
| `Parenthesis`        | `"JsdocTypeParenthesis"`        |
| `Conditional`        | `"JsdocTypeConditional"`        |
| `Predicate`          | `"JsdocTypePredicate"`          |
| `Asserts`            | `"JsdocTypeAsserts"`            |
| `AssertsPlain`       | `"JsdocTypeAssertsPlain"`       |
| `ReadonlyArray`      | `"JsdocTypeReadonlyArray"`      |
| `TemplateLiteral`    | `"JsdocTypeTemplateLiteral"`    |
| `Property`           | `"JsdocTypeProperty"`           |
| `IndexedAccessIndex` | `"JsdocTypeIndexedAccessIndex"` |

## Phase Summaries

### Phase 1: Foundation (Token + Lexer + Precedence)

- Token: 12-byte Copy type (start, end, kind). 3 bytes padding reserved for future flags.
- Lexer: Zero-copy, 1-token lookahead, base_offset addition, loose mode (NaN/Infinity).
  Reserved word handling: exact match only for keywords, `"functionBar"` → `Identifier`.
- Precedence: `#[repr(u8)]` enum, 21 levels (All → SpecialTypes). `PartialOrd` for `>` comparison.

### Phase 2: Pratt Parser Core

- `match` jump table for O(1) dispatch (eliminates parslet array scanning).
- Prefix parse (`parse_prefix_type`): 26-branch match statement.
- Infix precedence lookup (`cur_infix_precedence`): 13-branch match statement.
- Infix parse (`parse_infix_type`): 13-branch match statement.
- Helpers: `cur_kind()`, `bump()`, `expect()` are `#[inline]`. Error paths are `#[cold]`.
  Error type is `OxcDiagnostic` (`TypeError` is not used).
- Checkpoint/Lookahead: Disambiguating `(T)` vs `(a: T) => U`.
  Implemented via Lexer `save()/restore()` (separate from existing Checkpoint).
- Context flags: `disallow_conditional` (prevents nested conditional types inside extends clause).
- Mode difference handling: `if self.is_xxx()` conditional branches in match (no grammar table swapping).
- Intermediate nodes: `ParameterList` → `TypeFunction.parameters`,
  `ReadonlyProperty` → `TypeObjectField.readonly = true`.

### Phase 3: Type AST Definition

- Common enums: `ModifierPosition`, `GenericBrackets`, `QuoteStyle`, `ObjectSeparator`,
  `NamePathType`, `SpecialPathType`
- TypeNode enum: 35+ variants (basic, compound, name path, modifier, TS-specific,
  JSDoc/Closure-specific, object/function internals, intermediates)
- All struct definitions with spacing fields (for stringify roundtrip)
- JsdocType integration: `Parsed(ArenaBox<TypeNode>)` | `Raw(JsdocTypeSource)`

### Phase 4: Parse Function Implementation

- Prefix parse functions: 22 methods (all-mode common + mode-specific)
- Infix parse functions: 13 methods (with precedence)
- `match`-based dispatch, no parslet arrays

### Phase 5: Integration

- Type parse method: `ParserContext::parse_type_expression()` (internal, pub(crate))
- Helper functions: `traverse()`, `visitor_keys()`, `simplify()`, `get_parameters()`
  (provided as functions in `type_parser/` module)
- Diagnostics: `self.diagnostics.push(...)` directly (no TypeParser needed)
- JsdocTag integration: `parse_generic_tag_body()` calls `self.parse_type_expression()`
- Serializer integration: `JsdocType::Parsed` output to JSON
- ParseOptions extension: `type_parse_mode`, `parse_types` (false = zero cost)
- No standalone `parse_type()` public API needed (no external consumers)

### Phase 6: Stringify

- `stringify_type()`: AST to string reconstruction. Used by fixers.
- Roundtrip guarantee: `parse(stringify(ast)) == ast`
- Spacing metadata fields ensure exact whitespace reproduction

## Implementation Order

```
Phase 1: Foundation
  1.1 Token -> 1.2 Lexer (with reserved words) -> 1.3 Precedence

Phase 2: Parser Core
  2.1 Parser methods -> 2.2 Core loop -> 2.3 Checkpoint/Lookahead
                                          -> 2.4 Context flags

Phase 3: AST Definition
  3.1 TypeNode enum (all variants + intermediates)
  3.2 All struct definitions (with spacing fields)
  3.3 JsdocType integration + NodeKind integration

Phase 4: Parse Functions
  4.1 All-mode common -> 4.2 JSDoc -> 4.3 Closure -> 4.4 TypeScript

Phase 5: Integration
  5.1 parse_type_expression() -> 5.2 traverse(), visitor_keys, simplify(), get_parameters()
  5.3 JsdocTag integration -> 5.4 Serializer

Phase 6: Stringify
  6.1 Basic stringify -> 6.2 Spacing support -> 6.3 Roundtrip verification
```

## Test Strategy

351+ fixtures from `refers/jsdoc-type-pratt-parser/test/fixtures/` are ported
to Rust to verify identical AST output for the same inputs.

| Level                   | Tests     | Description                                     |
| ----------------------- | --------- | ----------------------------------------------- |
| L1: Lexer unit          | ~10       | Token generation                                |
| L2: Parse function unit | ~200      | Per-syntax input/output                         |
| L3: Mode error          | ~50       | Unsupported syntax errors per mode              |
| L4: Stringify roundtrip | ~40       | parse → stringify → reparse                     |
| L5: JS integration      | ~1000     | Dynamic comparison with jsdoc-type-pratt-parser |
| L6: Span offset         | ~10       | base_offset absolute position                   |
| L7: Reserved words      | ~15       | `functionBar` etc. as identifiers               |
| **Total**               | **~1325** |                                                 |

## Performance Design

### Patterns Referenced from oxc_parser

| #   | Pattern                                     | Applied To                                                            |
| --- | ------------------------------------------- | --------------------------------------------------------------------- |
| 1   | `match` jump table for O(1) dispatch        | `parse_prefix_type()`, `cur_infix_precedence()`, `parse_infix_type()` |
| 2   | Token as Copy type (12 bytes) bit-packed    | `Token { start, end, kind }` — register ops only                      |
| 3   | `token_text()` lazy retrieval on demand     | Most tokens need only `kind`                                          |
| 4   | `#[inline]` for hot path functions          | `cur_kind()`, `bump()`, `at()`, `is_jsdoc()`                          |
| 5   | `#[cold] #[inline(never)]` for error paths  | `error_no_prefix()`, `error_expected()`                               |
| 6   | `ArenaVec::with_capacity()` pre-sized       | Union elements, Generic parameters, Object fields                     |
| 7   | Ownership chains avoid Clone                | `left` move semantics                                                 |
| 8   | Mode branches as `#[inline]` bool functions | `is_jsdoc()`, `is_typescript()` — LLVM eliminates dead branches       |
| 9   | Zero indirect calls                         | No function pointers/vtables, all direct method calls                 |
| 10  | Absolute span offsets                       | `base_offset` added once in Lexer                                     |

### Performance Goals

- **10x+ faster** than jsdoc-type-pratt-parser (JS)
- O(1) dispatch per token (no parslet array scan)
- Token passing: register operations only (12-byte Copy)
- All AST nodes in arena allocator
- `parse_types: false` = zero cost (if branch only, no feature flag)
- Seamless integration with ox-jsdoc's existing parser pipeline,
  with no additional data conversion or copies to store in `JsdocTag.parsed_type`
