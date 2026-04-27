/**
 * Public type definitions for `@ox-jsdoc/decoder`.
 *
 * The Phase 1.1d hand-written decoder ships with a minimal type surface;
 * the full per-node type definitions will be code-generated in Phase 4
 * alongside the runtime classes.
 *
 * @author kazuya kawaguchi (a.k.a. kazupon)
 * @license MIT
 */

/** Range tuple `[absolutePosStart, absolutePosEnd]`. */
export type Range = [number, number]

/** Marker interface implemented by every Remote* lazy class. */
export interface RemoteNode {
  readonly type: string
  readonly range: Range
  readonly parent: RemoteNode | null
  toJSON(): Record<string, unknown>
}

/** `Array` subclass returned by every node-list getter. */
export class RemoteNodeList extends Array<RemoteNode> {}

/** Empty-array singleton (use `length === 0` to branch). */
export const EMPTY_NODE_LIST: RemoteNodeList

/** Construction options for {@link RemoteSourceFile}. */
export interface RemoteSourceFileOptions {
  /**
   * When the buffer's `compat_mode` flag is set, switch `toJSON()` and
   * compat-mode-only field accessors to emit `""` instead of `null` for
   * absent optional strings (rawType, name, namepathOrURL, text). Mirrors
   * the Rust serializer's `SerializeOptions.empty_string_for_null` for
   * jsdoccomment parity. Has no effect on basic-mode buffers.
   */
  emptyStringForNull?: boolean
}

/** Root of the decoder; constructed once per Binary AST buffer. */
export class RemoteSourceFile {
  constructor(buffer: ArrayBuffer | ArrayBufferView, options?: RemoteSourceFileOptions)

  readonly view: DataView
  readonly compatMode: boolean
  readonly emptyStringForNull: boolean
  readonly extendedDataOffset: number
  readonly nodesOffset: number
  readonly nodeCount: number
  readonly rootCount: number

  readonly asts: ReadonlyArray<RemoteNode | null>

  getString(idx: number): string | null
  getStringByField(offset: number, length: number): string | null
  getRootBaseOffset(rootIndex: number): number
  getNode(nodeIndex: number, parent: RemoteNode | null, rootIndex?: number): RemoteNode | null
}

/** Recursively convert a lazy node into a plain JSON object. */
export function toPlainObject(node: unknown): unknown

// ---------------------------------------------------------------------------
// Comment AST classes
// ---------------------------------------------------------------------------

export class RemoteJsdocBlock implements RemoteNode {
  readonly type: 'JsdocBlock'
  readonly range: Range
  readonly parent: RemoteNode | null
  readonly description: string | null
  readonly delimiter: string
  readonly postDelimiter: string
  readonly terminal: string
  readonly lineEnd: string
  readonly initial: string
  readonly delimiterLineBreak: string
  readonly preterminalLineBreak: string
  readonly descriptionLines: RemoteNodeList
  readonly tags: RemoteNodeList
  readonly inlineTags: RemoteNodeList
  // compat-mode-only line metadata (jsdoccomment compatibility). Each is
  // `null` when the buffer is not in compat mode; absent indices use `null`.
  readonly endLine: number | null
  readonly descriptionStartLine: number | null
  readonly descriptionEndLine: number | null
  readonly lastDescriptionLine: number | null
  readonly hasPreterminalDescription: number | null
  readonly hasPreterminalTagDescription: number | null
  /**
   * Raw description slice (with `*` prefix and blank lines intact).
   * `null` outside compat mode (the wire field doesn't exist) or when
   * the block has no description.
   * See `design/008-oxlint-oxfmt-support/README.md` §4.3.
   */
  readonly descriptionRaw: string | null
  /**
   * Description text. When `preserveWhitespace` is `true`, blank lines
   * and indentation past the `* ` prefix are preserved (algorithm: see
   * design §3). When `false` (default), returns the compact view (same
   * as `description`).
   * Returns `null` when no description is present, or when
   * `preserveWhitespace=true` is requested on a basic-mode buffer.
   */
  descriptionText(preserveWhitespace?: boolean): string | null
  toJSON(): Record<string, unknown>
}

export class RemoteJsdocDescriptionLine implements RemoteNode {
  readonly type: 'JsdocDescriptionLine'
  readonly range: Range
  readonly parent: RemoteNode | null
  readonly description: string
  // compat-mode-only delimiter trio. `null` outside compat mode.
  readonly delimiter: string | null
  readonly postDelimiter: string | null
  readonly initial: string | null
  toJSON(): Record<string, unknown>
}

export class RemoteJsdocTag implements RemoteNode {
  readonly type: 'JsdocTag'
  readonly range: Range
  readonly parent: RemoteNode | null
  readonly optional: boolean
  readonly defaultValue: string | null
  readonly description: string | null
  readonly rawBody: string | null
  readonly tag: RemoteJsdocTagName | null
  readonly rawType: RemoteJsdocTypeSource | null
  readonly name: RemoteJsdocTagNameValue | null
  readonly parsedType: RemoteNode | null
  readonly body: RemoteNode | null
  readonly typeLines: RemoteNodeList
  readonly descriptionLines: RemoteNodeList
  readonly inlineTags: RemoteNodeList
  // compat-mode-only delimiter strings. `null` outside compat mode.
  readonly delimiter: string | null
  readonly postDelimiter: string | null
  readonly postTag: string | null
  readonly postType: string | null
  readonly postName: string | null
  readonly initial: string | null
  readonly lineEnd: string | null
  /**
   * Raw description slice (with `*` prefix and blank lines intact).
   * `null` outside compat mode (the wire field doesn't exist) or when
   * the tag has no description body.
   * See `design/008-oxlint-oxfmt-support/README.md` §4.3.
   */
  readonly descriptionRaw: string | null
  /**
   * Description text. When `preserveWhitespace` is `true`, blank lines
   * and indentation past the `* ` prefix are preserved (algorithm: see
   * design §3). When `false` (default), returns the compact view (same
   * as `description`).
   * Returns `null` when no description is present, or when
   * `preserveWhitespace=true` is requested on a basic-mode buffer.
   */
  descriptionText(preserveWhitespace?: boolean): string | null
  toJSON(): Record<string, unknown>
}

export class RemoteJsdocTagName implements RemoteNode {
  readonly type: 'JsdocTagName'
  readonly range: Range
  readonly parent: RemoteNode | null
  readonly value: string
  toJSON(): Record<string, unknown>
}

export class RemoteJsdocTagNameValue implements RemoteNode {
  readonly type: 'JsdocTagNameValue'
  readonly range: Range
  readonly parent: RemoteNode | null
  readonly raw: string
  toJSON(): Record<string, unknown>
}

export class RemoteJsdocTypeSource implements RemoteNode {
  readonly type: 'JsdocTypeSource'
  readonly range: Range
  readonly parent: RemoteNode | null
  readonly raw: string
  toJSON(): Record<string, unknown>
}

export class RemoteJsdocTypeLine implements RemoteNode {
  readonly type: 'JsdocTypeLine'
  readonly range: Range
  readonly parent: RemoteNode | null
  readonly rawType: string
  // compat-mode-only delimiter trio. `null` outside compat mode.
  readonly delimiter: string | null
  readonly postDelimiter: string | null
  readonly initial: string | null
  toJSON(): Record<string, unknown>
}

export class RemoteJsdocInlineTag implements RemoteNode {
  readonly type: 'JsdocInlineTag'
  readonly range: Range
  readonly parent: RemoteNode | null
  readonly format: 'plain' | 'pipe' | 'space' | 'prefix' | 'unknown'
  readonly namepathOrURL: string | null
  readonly text: string | null
  readonly rawBody: string | null
  toJSON(): Record<string, unknown>
}

export class RemoteJsdocGenericTagBody implements RemoteNode {
  readonly type: 'JsdocGenericTagBody'
  readonly range: Range
  readonly parent: RemoteNode | null
  readonly hasDashSeparator: boolean
  readonly description: string | null
  toJSON(): Record<string, unknown>
}

export class RemoteJsdocBorrowsTagBody implements RemoteNode {
  readonly type: 'JsdocBorrowsTagBody'
  readonly range: Range
  readonly parent: RemoteNode | null
  toJSON(): Record<string, unknown>
}

export class RemoteJsdocRawTagBody implements RemoteNode {
  readonly type: 'JsdocRawTagBody'
  readonly range: Range
  readonly parent: RemoteNode | null
  readonly raw: string
  toJSON(): Record<string, unknown>
}

export class RemoteJsdocParameterName implements RemoteNode {
  readonly type: 'JsdocParameterName'
  readonly range: Range
  readonly parent: RemoteNode | null
  readonly optional: boolean
  readonly path: string
  readonly defaultValue: string | null
  toJSON(): Record<string, unknown>
}

export class RemoteJsdocNamepathSource implements RemoteNode {
  readonly type: 'JsdocNamepathSource'
  readonly range: Range
  readonly parent: RemoteNode | null
  readonly raw: string
  toJSON(): Record<string, unknown>
}

export class RemoteJsdocIdentifier implements RemoteNode {
  readonly type: 'JsdocIdentifier'
  readonly range: Range
  readonly parent: RemoteNode | null
  readonly name: string
  toJSON(): Record<string, unknown>
}

export class RemoteJsdocText implements RemoteNode {
  readonly type: 'JsdocText'
  readonly range: Range
  readonly parent: RemoteNode | null
  readonly value: string
  toJSON(): Record<string, unknown>
}

// ---------------------------------------------------------------------------
// TypeNode classes — minimal surface; richer per-class fields will arrive
// alongside Phase 4 codegen. For now they all implement RemoteNode.
// ---------------------------------------------------------------------------

export class RemoteTypeName implements RemoteNode {
  readonly type: 'TypeName'
  readonly range: Range
  readonly parent: RemoteNode | null
  readonly value: string
  toJSON(): Record<string, unknown>
}

export class RemoteTypeNumber implements RemoteNode {
  readonly type: 'TypeNumber'
  readonly range: Range
  readonly parent: RemoteNode | null
  readonly value: string
  toJSON(): Record<string, unknown>
}

export class RemoteTypeStringValue implements RemoteNode {
  readonly type: 'TypeStringValue'
  readonly range: Range
  readonly parent: RemoteNode | null
  readonly value: string
  toJSON(): Record<string, unknown>
}

export class RemoteTypeProperty implements RemoteNode {
  readonly type: 'TypeProperty'
  readonly range: Range
  readonly parent: RemoteNode | null
  readonly value: string
  toJSON(): Record<string, unknown>
}

export class RemoteTypeSpecialNamePath implements RemoteNode {
  readonly type: 'TypeSpecialNamePath'
  readonly range: Range
  readonly parent: RemoteNode | null
  readonly value: string
  toJSON(): Record<string, unknown>
}

// All remaining TypeNode classes follow the same RemoteNode contract; we
// declare them with a single shared base interface so the d.ts stays
// readable (the runtime classes carry the full per-class getters).

export interface RemoteTypeNode extends RemoteNode {}

export class RemoteTypeUnion implements RemoteTypeNode {
  readonly type: 'TypeUnion'
  readonly range: Range
  readonly parent: RemoteNode | null
  readonly elements: RemoteNodeList
  toJSON(): Record<string, unknown>
}
export class RemoteTypeIntersection implements RemoteTypeNode {
  readonly type: 'TypeIntersection'
  readonly range: Range
  readonly parent: RemoteNode | null
  readonly elements: RemoteNodeList
  toJSON(): Record<string, unknown>
}
export class RemoteTypeGeneric implements RemoteTypeNode {
  readonly type: 'TypeGeneric'
  readonly range: Range
  readonly parent: RemoteNode | null
  readonly brackets: number
  readonly dot: boolean
  readonly left: RemoteNode | null
  readonly elements: RemoteNodeList
  toJSON(): Record<string, unknown>
}
export class RemoteTypeFunction implements RemoteTypeNode {
  readonly type: 'TypeFunction'
  readonly range: Range
  readonly parent: RemoteNode | null
  readonly arrow: boolean
  readonly parenthesis: boolean
  readonly parameters: RemoteNode | null
  readonly returnType: RemoteNode | null
  readonly typeParameters: RemoteNode | null
  toJSON(): Record<string, unknown>
}
export class RemoteTypeObject implements RemoteTypeNode {
  readonly type: 'TypeObject'
  readonly range: Range
  readonly parent: RemoteNode | null
  readonly separator: number
  readonly elements: RemoteNodeList
  toJSON(): Record<string, unknown>
}

// The remaining TypeNode classes follow the same shape — declared with the
// shared RemoteTypeNode interface so consumers can rely on `.type` /
// `.range` / `.parent` / `.toJSON()`. The full per-class field list is
// tracked in `js-decoder.md` and will be reflected here at Phase 4.
export class RemoteTypeTuple implements RemoteTypeNode {
  readonly type: 'TypeTuple'
  readonly range: Range
  readonly parent: RemoteNode | null
  readonly elements: RemoteNodeList
  toJSON(): Record<string, unknown>
}
export class RemoteTypeParenthesis implements RemoteTypeNode {
  readonly type: 'TypeParenthesis'
  readonly range: Range
  readonly parent: RemoteNode | null
  readonly element: RemoteNode | null
  toJSON(): Record<string, unknown>
}
export class RemoteTypeNamePath implements RemoteTypeNode {
  readonly type: 'TypeNamePath'
  readonly range: Range
  readonly parent: RemoteNode | null
  readonly left: RemoteNode | null
  readonly right: RemoteNode | null
  toJSON(): Record<string, unknown>
}
export class RemoteTypeNullable implements RemoteTypeNode {
  readonly type: 'TypeNullable'
  readonly range: Range
  readonly parent: RemoteNode | null
  readonly element: RemoteNode | null
  toJSON(): Record<string, unknown>
}
export class RemoteTypeNotNullable implements RemoteTypeNode {
  readonly type: 'TypeNotNullable'
  readonly range: Range
  readonly parent: RemoteNode | null
  readonly element: RemoteNode | null
  toJSON(): Record<string, unknown>
}
export class RemoteTypeOptional implements RemoteTypeNode {
  readonly type: 'TypeOptional'
  readonly range: Range
  readonly parent: RemoteNode | null
  readonly element: RemoteNode | null
  toJSON(): Record<string, unknown>
}
export class RemoteTypeVariadic implements RemoteTypeNode {
  readonly type: 'TypeVariadic'
  readonly range: Range
  readonly parent: RemoteNode | null
  readonly element: RemoteNode | null
  toJSON(): Record<string, unknown>
}
export class RemoteTypeConditional implements RemoteTypeNode {
  readonly type: 'TypeConditional'
  readonly range: Range
  readonly parent: RemoteNode | null
  readonly checkType: RemoteNode | null
  readonly extendsType: RemoteNode | null
  readonly trueType: RemoteNode | null
  readonly falseType: RemoteNode | null
  toJSON(): Record<string, unknown>
}
export class RemoteTypeInfer implements RemoteTypeNode {
  readonly type: 'TypeInfer'
  readonly range: Range
  readonly parent: RemoteNode | null
  readonly element: RemoteNode | null
  toJSON(): Record<string, unknown>
}
export class RemoteTypeKeyOf implements RemoteTypeNode {
  readonly type: 'TypeKeyOf'
  readonly range: Range
  readonly parent: RemoteNode | null
  readonly element: RemoteNode | null
  toJSON(): Record<string, unknown>
}
export class RemoteTypeTypeOf implements RemoteTypeNode {
  readonly type: 'TypeTypeOf'
  readonly range: Range
  readonly parent: RemoteNode | null
  readonly element: RemoteNode | null
  toJSON(): Record<string, unknown>
}
export class RemoteTypeImport implements RemoteTypeNode {
  readonly type: 'TypeImport'
  readonly range: Range
  readonly parent: RemoteNode | null
  readonly element: RemoteNode | null
  toJSON(): Record<string, unknown>
}
export class RemoteTypePredicate implements RemoteTypeNode {
  readonly type: 'TypePredicate'
  readonly range: Range
  readonly parent: RemoteNode | null
  readonly left: RemoteNode | null
  readonly right: RemoteNode | null
  toJSON(): Record<string, unknown>
}
export class RemoteTypeAsserts implements RemoteTypeNode {
  readonly type: 'TypeAsserts'
  readonly range: Range
  readonly parent: RemoteNode | null
  readonly left: RemoteNode | null
  readonly right: RemoteNode | null
  toJSON(): Record<string, unknown>
}
export class RemoteTypeAssertsPlain implements RemoteTypeNode {
  readonly type: 'TypeAssertsPlain'
  readonly range: Range
  readonly parent: RemoteNode | null
  readonly element: RemoteNode | null
  toJSON(): Record<string, unknown>
}
export class RemoteTypeReadonlyArray implements RemoteTypeNode {
  readonly type: 'TypeReadonlyArray'
  readonly range: Range
  readonly parent: RemoteNode | null
  readonly element: RemoteNode | null
  toJSON(): Record<string, unknown>
}
export class RemoteTypeObjectField implements RemoteTypeNode {
  readonly type: 'TypeObjectField'
  readonly range: Range
  readonly parent: RemoteNode | null
  readonly optional: boolean
  readonly readonly: boolean
  readonly quote: number
  readonly key: RemoteNode | null
  readonly right: RemoteNode | null
  toJSON(): Record<string, unknown>
}
export class RemoteTypeJsdocObjectField implements RemoteTypeNode {
  readonly type: 'TypeJsdocObjectField'
  readonly range: Range
  readonly parent: RemoteNode | null
  readonly key: RemoteNode | null
  readonly right: RemoteNode | null
  toJSON(): Record<string, unknown>
}
export class RemoteTypeIndexedAccessIndex implements RemoteTypeNode {
  readonly type: 'TypeIndexedAccessIndex'
  readonly range: Range
  readonly parent: RemoteNode | null
  readonly element: RemoteNode | null
  toJSON(): Record<string, unknown>
}
export class RemoteTypeCallSignature implements RemoteTypeNode {
  readonly type: 'TypeCallSignature'
  readonly range: Range
  readonly parent: RemoteNode | null
  readonly parameters: RemoteNode | null
  readonly returnType: RemoteNode | null
  readonly typeParameters: RemoteNode | null
  toJSON(): Record<string, unknown>
}
export class RemoteTypeConstructorSignature implements RemoteTypeNode {
  readonly type: 'TypeConstructorSignature'
  readonly range: Range
  readonly parent: RemoteNode | null
  readonly parameters: RemoteNode | null
  readonly returnType: RemoteNode | null
  readonly typeParameters: RemoteNode | null
  toJSON(): Record<string, unknown>
}
export class RemoteTypeTypeParameter implements RemoteTypeNode {
  readonly type: 'TypeTypeParameter'
  readonly range: Range
  readonly parent: RemoteNode | null
  readonly elements: RemoteNodeList
  toJSON(): Record<string, unknown>
}
export class RemoteTypeParameterList implements RemoteTypeNode {
  readonly type: 'TypeParameterList'
  readonly range: Range
  readonly parent: RemoteNode | null
  readonly elements: RemoteNodeList
  toJSON(): Record<string, unknown>
}
export class RemoteTypeReadonlyProperty implements RemoteTypeNode {
  readonly type: 'TypeReadonlyProperty'
  readonly range: Range
  readonly parent: RemoteNode | null
  readonly element: RemoteNode | null
  toJSON(): Record<string, unknown>
}
export class RemoteTypeKeyValue implements RemoteTypeNode {
  readonly type: 'TypeKeyValue'
  readonly range: Range
  readonly parent: RemoteNode | null
  readonly optional: boolean
  readonly variadic: boolean
  readonly key: string
  readonly right: RemoteNode | null
  toJSON(): Record<string, unknown>
}
export class RemoteTypeIndexSignature implements RemoteTypeNode {
  readonly type: 'TypeIndexSignature'
  readonly range: Range
  readonly parent: RemoteNode | null
  readonly key: string
  readonly right: RemoteNode | null
  toJSON(): Record<string, unknown>
}
export class RemoteTypeMappedType implements RemoteTypeNode {
  readonly type: 'TypeMappedType'
  readonly range: Range
  readonly parent: RemoteNode | null
  readonly key: string
  readonly right: RemoteNode | null
  toJSON(): Record<string, unknown>
}
export class RemoteTypeMethodSignature implements RemoteTypeNode {
  readonly type: 'TypeMethodSignature'
  readonly range: Range
  readonly parent: RemoteNode | null
  readonly quote: number
  readonly hasParameters: boolean
  readonly hasTypeParameters: boolean
  readonly name: string
  toJSON(): Record<string, unknown>
}
export class RemoteTypeTemplateLiteral implements RemoteTypeNode {
  readonly type: 'TypeTemplateLiteral'
  readonly range: Range
  readonly parent: RemoteNode | null
  readonly literalCount: number
  readonly literals: string[]
  literal(index: number): string
  toJSON(): Record<string, unknown>
}
export class RemoteTypeSymbol implements RemoteTypeNode {
  readonly type: 'TypeSymbol'
  readonly range: Range
  readonly parent: RemoteNode | null
  readonly hasElement: boolean
  readonly value: string
  readonly element: RemoteNode | null
  toJSON(): Record<string, unknown>
}
export class RemoteTypeNull implements RemoteTypeNode {
  readonly type: 'TypeNull'
  readonly range: Range
  readonly parent: RemoteNode | null
  toJSON(): Record<string, unknown>
}
export class RemoteTypeUndefined implements RemoteTypeNode {
  readonly type: 'TypeUndefined'
  readonly range: Range
  readonly parent: RemoteNode | null
  toJSON(): Record<string, unknown>
}
export class RemoteTypeAny implements RemoteTypeNode {
  readonly type: 'TypeAny'
  readonly range: Range
  readonly parent: RemoteNode | null
  toJSON(): Record<string, unknown>
}
export class RemoteTypeUnknown implements RemoteTypeNode {
  readonly type: 'TypeUnknown'
  readonly range: Range
  readonly parent: RemoteNode | null
  toJSON(): Record<string, unknown>
}
export class RemoteTypeUniqueSymbol implements RemoteTypeNode {
  readonly type: 'TypeUniqueSymbol'
  readonly range: Range
  readonly parent: RemoteNode | null
  toJSON(): Record<string, unknown>
}

export class RemoteNodeListNode implements RemoteNode {
  readonly type: 'NodeList'
  readonly range: Range
  readonly parent: RemoteNode | null
  readonly elementCount: number
  readonly children: RemoteNode[]
  toJSON(): Record<string, unknown>
}

/**
 * Visitor keys for every Remote* node kind (60 = 15 Comment AST + 45
 * TypeNode). Maps each `type` name to the traversable child property names
 * in canonical visit order; spread directly into ESLint / `estraverse`
 * key maps. See the JS doc-comment in `index.js` for ox-jsdoc-vs-jsdoccomment
 * differences (notably `JsdocTag.tag` / `rawType` / `name` / `body` are
 * child nodes here, not strings).
 */
export const jsdocVisitorKeys: Readonly<Record<string, readonly string[]>>

