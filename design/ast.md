# ox-jsdoc AST Definition

AST definition for JSDoc comments based on the EBNF grammar (`design/syntax.ebnf`).
Follows oxc's arena allocation design (`Box<'a, T>`, `Vec<'a, T>`, `Span`).

> The definitions below serve as a design document for the Rust implementation
> and may differ in notation from the actual code.
> `'a` represents the arena allocator lifetime.

---

## Design Principles

1. **Preserve tree structure** — Represent comment structure as a tree, not a flat Doclet like jsdoc
2. **Preserve type expression AST** — Retain the type expression tree instead of discarding it after parsing like Catharsis
3. **Span on every node** — Track source positions for diagnostics and linter integration
4. **oxc style** — Arena allocation, `#[repr(C)]`, uniform-size enums, visitor pattern support
5. **Custom tag support** — Unknown tags are represented generically in the AST

---

## 1. Top Level: JSDocComment

EBNF: `JSDocComment = "/**" Body "*/" ;`

```rust
/// Root node representing an entire JSDoc comment.
/// The result of parsing a BlockComment (`/** ... */`) detected by oxc-parser.
#[repr(C)]
pub struct JSDocComment<'a> {
    pub span: Span,
    /// Description text at the beginning of the comment body (before any tags)
    pub description: Option<Box<'a, Description<'a>>>,
    /// List of block tags (`@param`, `@returns`, etc.)
    pub tags: Vec<'a, BlockTag<'a>>,
}
```

---

## 2. Description Text: Description

EBNF: `Description = DescriptionText { InlineTag DescriptionText } ;`

Description text is represented as an interleaved sequence of plain text and inline tags.

```rust
/// Description text. An interleaved sequence of plain text and inline tags.
/// Example: `"This is a {@link Foo} description"`
///   → [Text("This is a "), InlineTag({@link Foo}), Text(" description")]
#[repr(C)]
pub struct Description<'a> {
    pub span: Span,
    /// Interleaved sequence of text and inline tags
    pub parts: Vec<'a, DescriptionPart<'a>>,
}

/// A component of description text
#[repr(C, u8)]
pub enum DescriptionPart<'a> {
    /// Plain text portion
    Text(Box<'a, Text<'a>>),
    /// Inline tag (`{@link ...}`, etc.)
    InlineTag(Box<'a, InlineTag<'a>>),
    /// Fenced code block (``` ... ``` or ~~~ ... ~~~)
    /// Content is preserved as literal text; no tag/inline-tag parsing inside.
    FencedCodeBlock(Box<'a, FencedCodeBlock<'a>>),
    /// Inline code (`code` or `` `code` ``)
    /// Content is preserved as literal text; no tag parsing inside.
    InlineCode(Box<'a, InlineCode<'a>>),
}

/// Plain text node
#[repr(C)]
pub struct Text<'a> {
    pub span: Span,
    /// Text content (zero-copy reference to source text)
    pub value: &'a str,
}

/// Fenced code block (e.g. ``` ... ``` or ~~~ ... ~~~)
/// All content is literal — no tag or inline tag recognition inside.
#[repr(C)]
pub struct FencedCodeBlock<'a> {
    pub span: Span,
    /// Fence marker character ('`' or '~')
    pub fence_char: FenceChar,
    /// Number of fence characters (3 or more)
    pub fence_length: u32,
    /// Info string after opening fence (e.g. "javascript"), if any
    pub info: Option<Box<'a, Text<'a>>>,
    /// Raw content between opening and closing fence (zero-copy)
    pub content: &'a str,
}

/// Fence marker character
#[repr(u8)]
pub enum FenceChar {
    /// Backtick (`)
    Backtick,
    /// Tilde (~)
    Tilde,
}

/// Inline code (e.g. `code` or ``code with ` inside``)
/// Content is literal — no tag recognition inside.
#[repr(C)]
pub struct InlineCode<'a> {
    pub span: Span,
    /// Raw content between backtick delimiters (zero-copy)
    pub content: &'a str,
}
```

---

## 3. Block Tag: BlockTag

EBNF: `BlockTag = "@" TagName [ whitespace TagBody ] ;`
EBNF: `TagBody = [ TypeExpression ] [ ParameterName ] [ Separator ] [ Description ] ;`

```rust
/// A block tag (e.g. `@param {string} name - description`)
#[repr(C)]
pub struct BlockTag<'a> {
    pub span: Span,
    /// Tag name (without `@`. e.g. `"param"`, `"returns"`, `"customTag"`)
    pub tag_name: TagName<'a>,
    /// Type expression (the `{string}` part)
    pub type_expression: Option<Box<'a, TypeExpression<'a>>>,
    /// Parameter name or name path (the `name` or `[name=default]` part)
    pub name: Option<Box<'a, TagParameterName<'a>>>,
    /// Description text (the `- description` part)
    pub description: Option<Box<'a, Description<'a>>>,
}

/// Tag name
#[repr(C)]
pub struct TagName<'a> {
    pub span: Span,
    /// Tag name string (e.g. `"param"`, `"returns"`, `"customTag"`)
    pub value: &'a str,
}
```

---

## 4. Inline Tag: InlineTag

EBNF: `InlineTag = "{@" InlineTagName [ whitespace InlineTagBody ] "}" ;`

```rust
/// An inline tag (e.g. `{@link Foo}`, `{@type string}`)
#[repr(C)]
pub struct InlineTag<'a> {
    pub span: Span,
    /// Tag name (e.g. `"link"`, `"linkcode"`, `"type"`)
    pub tag_name: TagName<'a>,
    /// Interpreted body content
    pub body: InlineTagBody<'a>,
}

/// Inline tag body variants
#[repr(C, u8)]
pub enum InlineTagBody<'a> {
    /// `{@link namepath_or_url [text]}`
    Link(Box<'a, InlineLinkBody<'a>>),
    /// `{@type TypeExpr}`
    Type(Box<'a, TypeExpression<'a>>),
    /// Unknown inline tag — preserved as raw text
    Unknown(Box<'a, Text<'a>>),
}

/// Body of a link inline tag
#[repr(C)]
pub struct InlineLinkBody<'a> {
    pub span: Span,
    /// Link target (name path or URL)
    pub target: LinkTarget<'a>,
    /// Link text (optional)
    pub text: Option<Box<'a, Text<'a>>>,
}

/// Link target kind
#[repr(C, u8)]
pub enum LinkTarget<'a> {
    /// Name path (e.g. `MyClass#method`)
    NamePath(Box<'a, NamePath<'a>>),
    /// URL (e.g. `https://example.com`)
    URL(Box<'a, Text<'a>>),
}
```

---

## 5. Parameter Name: TagParameterName

EBNF: `ParameterName = OptionalParameter | RequiredParameter ;`

```rust
/// Tag parameter name (required or optional)
#[repr(C, u8)]
pub enum TagParameterName<'a> {
    /// Required parameter (e.g. `name`)
    Required(Box<'a, RequiredParameterName<'a>>),
    /// Optional parameter (e.g. `[name]`, `[name=default]`)
    Optional(Box<'a, OptionalParameterName<'a>>),
}

/// Required parameter name
#[repr(C)]
pub struct RequiredParameterName<'a> {
    pub span: Span,
    /// Name path
    pub name: NamePath<'a>,
}

/// Optional parameter name (`[name=default]`)
#[repr(C)]
pub struct OptionalParameterName<'a> {
    pub span: Span,
    /// Name path
    pub name: NamePath<'a>,
    /// Default value (raw text after `=`)
    pub default_value: Option<Box<'a, Text<'a>>>,
}
```

---

## 6. Name Path: NamePath

EBNF: `NamePath = NamePathSegment { ScopeOperator NamePathSegment } ;`

```rust
/// A name path (e.g. `MyClass`, `MyClass#method`, `module:foo.Bar~inner`)
#[repr(C)]
pub struct NamePath<'a> {
    pub span: Span,
    /// Path components
    pub segments: Vec<'a, NamePathComponent<'a>>,
}

/// A single component of a name path (scope operator + segment)
#[repr(C)]
pub struct NamePathComponent<'a> {
    pub span: Span,
    /// Scope operator (None for the first segment)
    pub operator: Option<ScopeOperator>,
    /// Segment
    pub segment: NamePathSegment<'a>,
}

/// Scope operator
#[repr(u8)]
pub enum ScopeOperator {
    /// `.` — static member
    Dot,
    /// `#` — instance member
    Hash,
    /// `~` — inner member
    Tilde,
}

/// Name path segment
#[repr(C, u8)]
pub enum NamePathSegment<'a> {
    /// Simple name (e.g. `MyClass`)
    Name(Box<'a, Identifier<'a>>),
    /// String literal name (e.g. `"special-name"`)
    StringLiteral(Box<'a, Text<'a>>),
    /// Event namespace (e.g. `event:click`)
    Event(Box<'a, Identifier<'a>>),
    /// Module prefix (e.g. `module:foo`)
    Module(Box<'a, Identifier<'a>>),
}

/// Identifier
#[repr(C)]
pub struct Identifier<'a> {
    pub span: Span,
    pub name: &'a str,
}
```

---

## 7. Type Expression: TypeExpression

EBNF: Corresponds to the entirety of section 5.
**Key difference from jsdoc — the tree structure is preserved.**

```rust
/// Complete type expression (including braces `{string|number}`)
#[repr(C)]
pub struct TypeExpression<'a> {
    pub span: Span,
    /// Type expression inside the braces
    pub type_expr: TypeExpr<'a>,
}

/// Type expression node (recursive tree structure)
#[repr(C, u8)]
pub enum TypeExpr<'a> {
    // --- Primitive literals ---
    /// `*` — any type
    AllLiteral(Box<'a, AllLiteral>),
    /// `null`
    NullLiteral(Box<'a, NullLiteral>),
    /// `undefined` or `void`
    UndefinedLiteral(Box<'a, UndefinedLiteral>),
    /// `?` — unknown type
    UnknownLiteral(Box<'a, UnknownLiteral>),
    /// String literal type (e.g. `'click'`)
    StringLiteral(Box<'a, StringLiteralType<'a>>),
    /// Numeric literal type (e.g. `42`)
    NumericLiteral(Box<'a, NumericLiteralType<'a>>),

    // --- Name ---
    /// Type name (e.g. `string`, `MyClass`, `module:foo.Bar`)
    Name(Box<'a, TypeName<'a>>),

    // --- Composite types ---
    /// Union type (e.g. `string|number`)
    Union(Box<'a, UnionType<'a>>),
    /// Generic type (e.g. `Array.<string>`, `Map<string, number>`)
    TypeApplication(Box<'a, TypeApplication<'a>>),
    /// Function type (e.g. `function(string): boolean`)
    Function(Box<'a, FunctionType<'a>>),
    /// Record type (e.g. `{key: string, value: number}`)
    Record(Box<'a, RecordType<'a>>),

    // --- Modified types ---
    /// Nullable type (e.g. `?string`)
    Nullable(Box<'a, NullableType<'a>>),
    /// Non-nullable type (e.g. `!string`)
    NonNullable(Box<'a, NonNullableType<'a>>),
    /// Optional type (e.g. `string=`, Closure Compiler style)
    Optional(Box<'a, OptionalType<'a>>),
    /// Rest/variadic type (e.g. `...string`)
    Variadic(Box<'a, VariadicType<'a>>),
    /// Array shorthand (e.g. `string[]`)
    ArrayShorthand(Box<'a, ArrayShorthandType<'a>>),

    // --- Grouping ---
    /// Parenthesized type (e.g. `(string|number)`)
    Parenthesized(Box<'a, ParenthesizedType<'a>>),
}
```

### 7.1 Primitive Literal Types

```rust
#[repr(C)]
pub struct AllLiteral {
    pub span: Span,
}

#[repr(C)]
pub struct NullLiteral {
    pub span: Span,
}

#[repr(C)]
pub struct UndefinedLiteral {
    pub span: Span,
    /// Distinguishes between `undefined` and `void`
    pub keyword: UndefinedKeyword,
}

#[repr(u8)]
pub enum UndefinedKeyword {
    Undefined,
    Void,
}

#[repr(C)]
pub struct UnknownLiteral {
    pub span: Span,
}

#[repr(C)]
pub struct StringLiteralType<'a> {
    pub span: Span,
    pub value: &'a str,
}

#[repr(C)]
pub struct NumericLiteralType<'a> {
    pub span: Span,
    pub raw: &'a str,
}
```

### 7.2 Type Name

```rust
/// Type name (e.g. `string`, `module:foo.Bar`)
#[repr(C)]
pub struct TypeName<'a> {
    pub span: Span,
    /// Qualified name parts (`.`-separated)
    pub parts: Vec<'a, Identifier<'a>>,
    /// Whether the `module:` prefix is present
    pub is_module: bool,
}
```

### 7.3 Composite Types

```rust
/// Union type (e.g. `string|number|boolean`)
#[repr(C)]
pub struct UnionType<'a> {
    pub span: Span,
    /// Union elements (two or more)
    pub elements: Vec<'a, TypeExpr<'a>>,
}

/// Generic/template type (e.g. `Array.<string>`, `Map<K, V>`)
#[repr(C)]
pub struct TypeApplication<'a> {
    pub span: Span,
    /// Base type name (e.g. `Array`, `Map`)
    pub name: TypeName<'a>,
    /// Type argument list
    pub type_arguments: Vec<'a, TypeExpr<'a>>,
    /// Whether dot notation is used (`Array.<T>` vs `Array<T>`)
    pub has_dot: bool,
}

/// Function type (e.g. `function(string, number): boolean`)
#[repr(C)]
pub struct FunctionType<'a> {
    pub span: Span,
    /// Parameter list (None if no signature is present)
    pub params: Option<Vec<'a, FunctionParam<'a>>>,
    /// Return type
    pub return_type: Option<Box<'a, TypeExpr<'a>>>,
    /// `this` context type
    pub this_type: Option<Box<'a, TypeExpr<'a>>>,
    /// `new` constructor type
    pub constructor_type: Option<Box<'a, TypeExpr<'a>>>,
}

/// Function type parameter
#[repr(C, u8)]
pub enum FunctionParam<'a> {
    /// Regular parameter
    Type(Box<'a, TypeExpr<'a>>),
    /// Rest parameter (`...T`)
    Rest(Box<'a, TypeExpr<'a>>),
}

/// Record type (e.g. `{key: string, value: number}`)
#[repr(C)]
pub struct RecordType<'a> {
    pub span: Span,
    /// Field list
    pub fields: Vec<'a, RecordField<'a>>,
}

/// Record type field
#[repr(C)]
pub struct RecordField<'a> {
    pub span: Span,
    /// Field name
    pub name: RecordFieldName<'a>,
    /// Field type (after `:`. Optional)
    pub value: Option<Box<'a, TypeExpr<'a>>>,
}

/// Record field name
#[repr(C, u8)]
pub enum RecordFieldName<'a> {
    Identifier(Box<'a, Identifier<'a>>),
    StringLiteral(Box<'a, StringLiteralType<'a>>),
    NumericLiteral(Box<'a, NumericLiteralType<'a>>),
}
```

### 7.4 Modified Types

```rust
/// Nullable type (`?string`)
#[repr(C)]
pub struct NullableType<'a> {
    pub span: Span,
    pub type_expr: TypeExpr<'a>,
}

/// Non-nullable type (`!string`)
#[repr(C)]
pub struct NonNullableType<'a> {
    pub span: Span,
    pub type_expr: TypeExpr<'a>,
}

/// Optional type (`string=`, Closure Compiler style)
#[repr(C)]
pub struct OptionalType<'a> {
    pub span: Span,
    pub type_expr: TypeExpr<'a>,
}

/// Variadic/rest type (`...string`)
#[repr(C)]
pub struct VariadicType<'a> {
    pub span: Span,
    pub type_expr: TypeExpr<'a>,
}

/// Array shorthand type (`string[]`)
#[repr(C)]
pub struct ArrayShorthandType<'a> {
    pub span: Span,
    pub element_type: TypeExpr<'a>,
}

/// Parenthesized type (`(string|number)`)
#[repr(C)]
pub struct ParenthesizedType<'a> {
    pub span: Span,
    pub type_expr: TypeExpr<'a>,
}
```

---

## 8. AST Node List and EBNF Mapping

| EBNF Production | AST Node | Notes |
|---|---|---|
| `JSDocComment` | `JSDocComment` | Root node |
| `Body` | `JSDocComment.{description, tags}` | Body itself has no dedicated node |
| `Description` | `Description` | Interleaved text + inline tags |
| `DescriptionText` | `Text` | Plain text |
| `FencedCodeBlock` | `FencedCodeBlock` | ``` ... ``` or ~~~ ... ~~~ |
| `InlineCode` | `InlineCode` | \`code\` or \`\`code\`\` |
| `BlockTag` | `BlockTag` | Generic tag structure (all components Optional) |
| `TagName` | `TagName` | Open-ended |
| `TagBody` | `BlockTag.{type_expression, name, description}` | TagBody itself has no dedicated node |
| `InlineTag` | `InlineTag` | `{@link ...}` etc. |
| `InlineTagBody` | `InlineTagBody` | 3 variants: Link / Type / Unknown |
| `ParameterName` | `TagParameterName` | 2 variants: Required / Optional |
| `OptionalParameter` | `OptionalParameterName` | `[name=default]` |
| `TypeExpression` | `TypeExpression` | Wrapper including braces |
| `TypeExpr` | `TypeExpr` | Recursive enum tree (17 variants) |
| `UnionType` | `UnionType` | Two or more elements |
| `TypeApplication` | `TypeApplication` | `Array.<T>` |
| `FunctionType` | `FunctionType` | `function(T): U` |
| `RecordType` | `RecordType` | `{k: T}` |
| `TypeName` | `TypeName` | `.`-separated qualified name |
| `NamePath` | `NamePath` | `.` / `#` / `~` separated path |

---

## 9. Visitor Pattern

Following oxc's `Visit`/`VisitMut`, visitor methods are defined for each AST node.

```rust
pub trait Visit<'a> {
    fn visit_jsdoc_comment(&mut self, comment: &JSDocComment<'a>);
    fn visit_description(&mut self, desc: &Description<'a>);
    fn visit_block_tag(&mut self, tag: &BlockTag<'a>);
    fn visit_inline_tag(&mut self, tag: &InlineTag<'a>);
    fn visit_tag_parameter_name(&mut self, name: &TagParameterName<'a>);
    fn visit_type_expression(&mut self, expr: &TypeExpression<'a>);
    fn visit_type_expr(&mut self, expr: &TypeExpr<'a>);
    fn visit_union_type(&mut self, union_type: &UnionType<'a>);
    fn visit_type_application(&mut self, app: &TypeApplication<'a>);
    fn visit_function_type(&mut self, func: &FunctionType<'a>);
    fn visit_record_type(&mut self, record: &RecordType<'a>);
    fn visit_name_path(&mut self, path: &NamePath<'a>);
    fn visit_identifier(&mut self, ident: &Identifier<'a>);
    fn visit_text(&mut self, text: &Text<'a>);
    fn visit_fenced_code_block(&mut self, block: &FencedCodeBlock<'a>);
    fn visit_inline_code(&mut self, code: &InlineCode<'a>);
}
```

---

## 10. Parse Example

### Input

```javascript
/**
 * Find a user.
 * See {@link UserService} for details.
 *
 * @param {string|number} id - The user ID
 * @param {Object.<string, *>} [options={}] - Options
 * @returns {Promise.<User>} The found user
 * @throws {NotFoundError} If the user is not found
 * @since 2.0.0
 * @deprecated Will be removed in 3.0.0. Use {@link findUserById} instead.
 */
```

### AST Output (Overview)

```
JSDocComment
├── description: Description
│   ├── Text("Find a user.\nSee ")
│   ├── InlineTag { tag_name: "link", body: Link { target: NamePath("UserService") } }
│   └── Text(" for details.")
├── tags[0]: BlockTag
│   ├── tag_name: "param"
│   ├── type_expression: TypeExpression
│   │   └── Union [Name("string"), Name("number")]
│   ├── name: Required(NamePath("id"))
│   └── description: Text("The user ID")
├── tags[1]: BlockTag
│   ├── tag_name: "param"
│   ├── type_expression: TypeExpression
│   │   └── TypeApplication { name: "Object", args: [Name("string"), AllLiteral] }
│   ├── name: Optional { name: NamePath("options"), default: Text("{}") }
│   └── description: Text("Options")
├── tags[2]: BlockTag
│   ├── tag_name: "returns"
│   ├── type_expression: TypeExpression
│   │   └── TypeApplication { name: "Promise", args: [Name("User")] }
│   └── description: Text("The found user")
├── tags[3]: BlockTag
│   ├── tag_name: "throws"
│   ├── type_expression: TypeExpression
│   │   └── Name("NotFoundError")
│   └── description: Text("If the user is not found")
├── tags[4]: BlockTag
│   ├── tag_name: "since"
│   └── description: Text("2.0.0")
└── tags[5]: BlockTag
    ├── tag_name: "deprecated"
    └── description: Description
        ├── Text("Will be removed in 3.0.0. Use ")
        ├── InlineTag { tag_name: "link", body: Link { target: NamePath("findUserById") } }
        └── Text(" instead.")
```

![ast-example.svg](./ast-example.svg)
