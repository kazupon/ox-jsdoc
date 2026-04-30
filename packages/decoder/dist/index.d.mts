//#region src/internal/types.d.ts
/**
 * Internal type definitions shared by every Remote* lazy class and the
 * helper functions that operate on their `_internal` state object.
 *
 * The public API surface (Remote* classes, RemoteSourceFile, etc.) is
 * declared separately in `../index.d.ts`. This file types the wiring
 * between the lazy classes and the byte-level decoder helpers.
 *
 * @author kazuya kawaguchi (a.k.a. kazupon)
 * @license MIT
 */
/**
 * Shape returned by every node's `toJSON()` method. Plain JSON object —
 * deliberately permissive so the recursive serializers in jsdoc.ts /
 * type-nodes.ts don't fight TypeScript over per-node shape variance.
 */
type RemoteJsonValue = string | number | boolean | null | RemoteJsonObject | RemoteJsonArray;
interface RemoteJsonObject {
  [key: string]: RemoteJsonValue | undefined;
}
type RemoteJsonArray = ReadonlyArray<RemoteJsonValue>;
/**
 * Marker shape implemented by every Remote* lazy class — kept loose so
 * helpers can pass instances around as `parent` / list children without
 * knowing the per-Kind discriminant.
 */
interface LazyNode {
  readonly type: string;
  readonly range: readonly [number, number];
  readonly parent: LazyNode | null;
  toJSON(): RemoteJsonObject;
}
/**
 * Public-facing surface of `RemoteSourceFile` consumed by helpers and
 * lazy node classes. Mirrors the runtime API but stays an interface so
 * it can be referenced before the class definition is loaded (avoids
 * circular type references between `source-file.ts` and `helpers.ts`).
 */
interface RemoteSourceFileLike {
  readonly view: DataView;
  readonly uint32View: Uint32Array;
  readonly compatMode: boolean;
  readonly emptyStringForNull: boolean;
  readonly extendedDataOffset: number;
  readonly nodesOffset: number;
  readonly nodeCount: number;
  readonly rootCount: number;
  getString(idx: number): string | null;
  getStringByField(offset: number, length: number): string | null;
  getStringByOffsetAndLength(offset: number, length: number): string;
  getRootBaseOffset(rootIndex: number): number;
  getRootSourceOffsetInData(rootIndex: number): number;
  getRootSourceText(rootIndex: number): string;
  sliceSourceText(rootIndex: number, start: number, end: number): string | null;
  getNode(nodeIndex: number, parent: LazyNode | null, rootIndex?: number): LazyNode | null;
}
/**
 * Shared `_internal` shape held by every Remote* lazy class. All helpers
 * in `helpers.ts` accept this type and read from it without knowing which
 * concrete class instance owns it.
 */
interface RemoteInternal {
  readonly view: DataView;
  /** Byte offset of this node record (= nodesOffset + index * 24). */
  readonly byteIndex: number;
  /** Node index (0 = sentinel). */
  readonly index: number;
  /** Index of the root this node belongs to (used for absolute range). */
  readonly rootIndex: number;
  /** The parent Remote* instance (null for roots). */
  readonly parent: LazyNode | null;
  readonly sourceFile: RemoteSourceFileLike;
}
/**
 * Constructor signature shared by every lazy class instantiated through
 * `decodeKindToClass`. Kept as a tuple so the kind dispatch table can
 * type-check `new Class(...)` calls.
 */
type LazyNodeConstructor = new (view: DataView, byteIndex: number, index: number, rootIndex: number, parent: LazyNode | null, sourceFile: RemoteSourceFileLike) => LazyNode;
//#endregion
//#region src/internal/source-file.d.ts
/**
 * Construction options for {@link RemoteSourceFile}.
 */
interface RemoteSourceFileOptions {
  /**
   * When the buffer's `compat_mode` flag is set, switch `toJSON()` and
   * compat-mode-only field accessors to emit `""` instead of `null` for
   * absent optional strings (rawType, name, namepathOrURL, text). Mirrors
   * the Rust serializer's `SerializeOptions.empty_string_for_null` for
   * jsdoccomment parity. Has no effect on basic-mode buffers.
   */
  emptyStringForNull?: boolean;
}
/**
 * Root of the lazy decoder. Construct one per Binary AST buffer.
 *
 * Public surface (used by Remote* node classes):
 * - `view`, `extendedDataOffset`, `nodesOffset`, `nodeCount`, `rootCount`,
 *   `compatMode` getters
 * - `getString(idx)` — String Offsets[idx] → resolved string (cached)
 * - `getRootBaseOffset(rootIndex)`
 * - `getNode(nodeIndex, parent, rootIndex)` — lazy class instance (cached)
 * - `asts` getter — array of root Remote* instances (or `null` for failures)
 */
declare class RemoteSourceFile implements RemoteSourceFileLike {
  #private;
  /**
   * Construct from a binary buffer (any `BufferSource`-compatible value).
   *
   * Throws when:
   * - the buffer is shorter than {@link HEADER_SIZE} bytes
   * - the major version disagrees with {@link SUPPORTED_MAJOR}
   *
   * `emptyStringForNull`: only effective when the buffer's `compat_mode`
   * flag is set. Switches `toJSON()` and compat-mode field accessors to
   * emit `""` instead of `null` for absent optional strings (rawType,
   * name, namepathOrURL, text). Mirrors the Rust serializer's
   * `SerializeOptions.empty_string_for_null` for jsdoccomment parity.
   */
  constructor(buffer: ArrayBuffer | ArrayBufferView, options?: RemoteSourceFileOptions);
  /** Underlying DataView. */
  get view(): DataView;
  /**
   * Underlying typed `Uint32Array` view aligned to the buffer start.
   * Index by `byteOffset >>> 2` for any 4-byte aligned u32 read; this is
   * 5–10× faster than `DataView.getUint32` in V8 hot paths.
   */
  get uint32View(): Uint32Array;
  /** Whether the buffer's `compat_mode` flag bit is set. */
  get compatMode(): boolean;
  /** Whether `null` optional strings are emitted as `""` in compat-mode. */
  get emptyStringForNull(): boolean;
  /** Byte offset of the Extended Data section. */
  get extendedDataOffset(): number;
  /** Byte offset of the Nodes section. */
  get nodesOffset(): number;
  /** Total number of node records (including the `node[0]` sentinel). */
  get nodeCount(): number;
  /** Number of roots N. */
  get rootCount(): number;
  /**
   * Resolve the string at `idx` (returns `null` for the
   * `STRING_PAYLOAD_NONE_SENTINEL` (`0x3FFF_FFFF`) sentinel). Used by
   * string-leaf nodes (TypeTag::String payload) and the diagnostics
   * section.
   *
   * Cached on first lookup so repeated reads are O(1).
   */
  getString(idx: number): string | null;
  /**
   * Resolve a `StringField` `(offset, length)` pair into the underlying
   * string. Returns `null` when the field is the `NONE` sentinel
   * (`offset === STRING_FIELD_NONE_OFFSET`). Used by Extended Data string
   * slots which embed `(offset, length)` directly.
   *
   * Cache key uses a high-bit-set form of `offset` so it never collides
   * with `getString(idx)` cache entries (string-leaf path uses small
   * indices, ED path uses byte offsets — both fit in u32 and overlap).
   */
  getStringByField(offset: number, length: number): string | null;
  /**
   * Resolve a Path B-leaf inline `(offset, length)` pair into the underlying
   * string. Always returns a real `&str` (never `null`) — encoders only
   * emit `TypeTag::StringInline` for present, non-empty short strings.
   *
   * Reuses the same cache-key disambiguation as `getStringByField` (offset
   * is tagged with the sign bit) so inline-path lookups never collide with
   * String-Offsets-table lookups.
   */
  getStringByOffsetAndLength(offset: number, length: number): string;
  /**
   * Get the `base_offset` for the i-th root (used to compute absolute ranges).
   */
  getRootBaseOffset(rootIndex: number): number;
  /**
   * Get the `source_offset_in_data` (byte offset where this root's source
   * text starts inside the String Data section) for the i-th root.
   * Used by `descriptionRaw` getters that need to slice the source text
   * by `(start, end)` byte offsets.
   */
  getRootSourceOffsetInData(rootIndex: number): number;
  /**
   * Slice the source text region for `rootIndex` at the given
   * `(start, end)` source-text-relative UTF-8 byte offsets. Returns
   * `null` for the `(0, 0)` sentinel, for `start > end`, or when the
   * slice would extend past the buffer.
   *
   * Used by `descriptionRaw` getters on `RemoteJsdocBlock` /
   * `RemoteJsdocTag` (compat-mode wire field per
   * `design/008-oxlint-oxfmt-support/README.md` §4.3).
   */
  sliceSourceText(rootIndex: number, start: number, end: number): string | null;
  /**
   * Return the complete source text for one root.
   */
  getRootSourceText(rootIndex: number): string;
  /**
   * Build (or fetch from cache) the lazy class instance for a node.
   *
   * Returns `null` for the sentinel (node index 0).
   */
  getNode(nodeIndex: number, parent: LazyNode | null, rootIndex?: number): LazyNode | null;
  /**
   * AST root for each entry in the Root Index array. Yields `null` for
   * entries with `node_index === 0` (parse failure sentinel) and the
   * matching lazy class instance otherwise.
   */
  get asts(): ReadonlyArray<LazyNode | null>;
}
//#endregion
//#region src/internal/node-list.d.ts
/**
 * `Array` subclass returned by every "node list" getter. Inheriting from
 * `Array` gives us `length` / `map` / `filter` / `forEach` etc. for free;
 * indexed access (`list[i]`) returns lazy class instances built up front.
 */
declare class RemoteNodeList extends Array<LazyNode> {}
/**
 * Empty singleton — every "no children" getter returns this so callers can
 * branch on `length === 0` without allocating.
 */
declare const EMPTY_NODE_LIST: RemoteNodeList;
//#endregion
//#region src/internal/inspect.d.ts
/**
 * Shared `Symbol.for('nodejs.util.inspect.custom')` helper.
 *
 * Returning a plain object whose prototype is set to an empty named class
 * makes `console.log(node)` print the class label (e.g. `JsdocBlock { ... }`)
 * in Node-family runtimes. Same trick as oxc raw transfer.
 *
 * In browsers `Symbol.for('nodejs.util.inspect.custom')` is harmless (the
 * key just becomes another property on the object).
 *
 * @author kazuya kawaguchi (a.k.a. kazupon)
 * @license MIT
 */
declare const inspectSymbol: unique symbol;
//#endregion
//#region src/internal/nodes/jsdoc.d.ts
/**
 * Minimal interface needed by the source[] reconstruction helpers — every
 * lazy class that owns a `range` / `parent` / `rootIndex` / `sourceFile`
 * tuple satisfies it. `parent` is the generic `LazyNode | null` (we only
 * need `.range` from it, which every LazyNode exposes).
 */
interface SourceLikeNode {
  readonly range: readonly [number, number];
  readonly parent: LazyNode | null;
  readonly rootIndex: number;
  readonly sourceFile: RemoteSourceFileLike;
}
/**
 * `JsdocInlineTagFormat` numeric → string label.
 * Mirrors Rust's `JsdocInlineTagFormat` enum order.
 */
declare const INLINE_TAG_FORMATS: readonly ["plain", "pipe", "space", "prefix", "unknown"];
type InlineTagFormat = (typeof INLINE_TAG_FORMATS)[number];
/**
 * `JsdocBlock` (Kind 0x01) — root of every parsed `/** ... *​/` comment.
 */
declare class RemoteJsdocBlock implements LazyNode, SourceLikeNode {
  #private;
  readonly type = "JsdocBlock";
  constructor(view: DataView, byteIndex: number, index: number, rootIndex: number, parent: LazyNode | null, sourceFile: RemoteSourceFileLike);
  get range(): readonly [number, number];
  get parent(): LazyNode | null;
  get sourceFile(): RemoteSourceFileLike;
  get rootIndex(): number;
  /** Top-level description string (`null` when absent). The
   * `emptyStringForNull` option only affects `toJSON()` output. */
  get description(): string | null;
  /** Source-preserving `*` line-prefix delimiter. */
  get delimiter(): string;
  /** Source-preserving space after `*`. */
  get postDelimiter(): string;
  /** Source-preserving `*​/` terminal. */
  get terminal(): string;
  /** Source-preserving line-end characters. */
  get lineEnd(): string;
  /** Indentation before the leading `*`. */
  get initial(): string;
  /** Line-break right after `/**`. */
  get delimiterLineBreak(): string;
  /** Line-break right before `*​/`. */
  get preterminalLineBreak(): string;
  /** Total number of LogicalLines in this comment (compat-mode only). */
  get endLine(): number | null;
  /** Index of the first description line, or `null` when absent. */
  get descriptionStartLine(): number | null;
  /** Index of the last description line, or `null` when absent. */
  get descriptionEndLine(): number | null;
  /** Description-boundary index (jsdoccomment's `lastDescriptionLine` —
   * actually the index of the first tag/end line). `null` when absent. */
  get lastDescriptionLine(): number | null;
  /** `1` when block description text exists on the `*​/` line. */
  get hasPreterminalDescription(): number | null;
  /** `1` when tag description text exists on the `*​/` line; `null` when not
   * applicable (no active lastTag at end). */
  get hasPreterminalTagDescription(): number | null;
  /**
   * Raw description slice (with `*` prefix and blank lines intact).
   * Returns `null` when the buffer was not parsed with
   * `preserveWhitespace: true` (the per-node
   * `has_description_raw_span` Common Data bit is clear), or when the
   * block has no description.
   *
   * Phase 5 layout: the span sits at the **last 8 bytes** of the ED
   * record (offset = `compatMode ? 90 : 68` = the basic / compat ED size).
   * See `design/008-oxlint-oxfmt-support/README.md` §4.2 / §4.3.
   */
  get descriptionRaw(): string | null;
  /**
   * Description text. When `preserveWhitespace` is `true`, blank lines
   * and indentation past the `* ` prefix are preserved (algorithm: see
   * `parsedPreservingWhitespace` / design §3). When `false` or omitted,
   * returns the compact view (`description` getter).
   *
   * Returns `null` when no description is present, or when
   * `preserveWhitespace=true` is requested on a buffer that wasn't
   * parsed with the matching `preserveWhitespace: true` parse option.
   */
  descriptionText(preserveWhitespace?: boolean): string | null;
  /** Top-level description lines. */
  get descriptionLines(): RemoteNodeList;
  /** Block tags. */
  get tags(): RemoteNodeList;
  /** Inline tags found inside the top-level description. */
  get inlineTags(): RemoteNodeList;
  toJSON(): RemoteJsonObject;
  [inspectSymbol](): object;
}
/**
 * `JsdocDescriptionLine` (Kind 0x02). Both basic and compat modes store
 * `description` as the leading StringField of the Extended Data record.
 */
declare class RemoteJsdocDescriptionLine implements LazyNode, SourceLikeNode {
  #private;
  readonly type = "JsdocDescriptionLine";
  constructor(view: DataView, byteIndex: number, index: number, rootIndex: number, parent: LazyNode | null, sourceFile: RemoteSourceFileLike);
  get range(): readonly [number, number];
  get parent(): LazyNode | null;
  get sourceFile(): RemoteSourceFileLike;
  get rootIndex(): number;
  /** Description content. Basic mode reads the String payload (Node Data);
   * compat mode reads byte 0-5 of the Extended Data record. */
  get description(): string;
  /** Source-preserving `*` line-prefix (compat-mode only; `null` otherwise). */
  get delimiter(): string | null;
  /** Source-preserving space after `*` (compat-mode only). */
  get postDelimiter(): string | null;
  /** Indentation before the leading `*` (compat-mode only). */
  get initial(): string | null;
  toJSON(): RemoteJsonObject;
  [inspectSymbol](): object;
}
/**
 * `JsdocTag` (Kind 0x03) — one block tag (e.g. `@param`).
 */
declare class RemoteJsdocTag implements LazyNode, SourceLikeNode {
  #private;
  readonly type = "JsdocTag";
  constructor(view: DataView, byteIndex: number, index: number, rootIndex: number, parent: LazyNode | null, sourceFile: RemoteSourceFileLike);
  get range(): readonly [number, number];
  get parent(): LazyNode | null;
  get sourceFile(): RemoteSourceFileLike;
  get rootIndex(): number;
  /** `bit0` of Common Data — was the tag wrapped in `[...]`? */
  get optional(): boolean;
  /** Default value parsed from `[id=foo]` syntax. */
  get defaultValue(): string | null;
  /** Joined description text. */
  get description(): string | null;
  /**
   * Raw description slice (with `*` prefix and blank lines intact).
   * Returns `null` when the buffer was not parsed with
   * `preserveWhitespace: true` (the per-node
   * `has_description_raw_span` Common Data bit is clear), or when the
   * tag has no description.
   *
   * Phase 5 layout: the span sits at the **last 8 bytes** of the ED
   * record (offset = `compatMode ? 80 : 38` = the basic / compat ED size).
   * See `design/008-oxlint-oxfmt-support/README.md` §4.2 / §4.3.
   */
  get descriptionRaw(): string | null;
  /**
   * Description text. Identical contract to
   * `RemoteJsdocBlock.descriptionText`.
   */
  descriptionText(preserveWhitespace?: boolean): string | null;
  /** Raw body when the tag uses the `Raw` body variant. */
  get rawBody(): string | null;
  /** Mandatory tag-name child (visitor index 0 — the `@name` token). */
  get tag(): LazyNode | null;
  /** Raw `{...}` type source (visitor index 1). */
  get rawType(): LazyNode | null;
  /** Tag-name value (visitor index 2). */
  get name(): LazyNode | null;
  /** `parsedType` child (visitor index 3) — any TypeNode variant. */
  get parsedType(): LazyNode | null;
  /** Body child (visitor index 4) — Generic / Borrows / Raw variant. */
  get body(): LazyNode | null;
  /** Source-preserving type lines. */
  get typeLines(): RemoteNodeList;
  /** Source-preserving description lines. */
  get descriptionLines(): RemoteNodeList;
  /** Inline tags found in this tag's description. */
  get inlineTags(): RemoteNodeList;
  /** Source-preserving `*` line-prefix (compat-mode only). */
  get delimiter(): string | null;
  /** Source-preserving space after `*` (compat-mode only). */
  get postDelimiter(): string | null;
  /** Whitespace after the `@name` token (compat-mode only). */
  get postTag(): string | null;
  /** Whitespace after the `{type}` source (compat-mode only). */
  get postType(): string | null;
  /** Whitespace after the name token (compat-mode only). */
  get postName(): string | null;
  /** Indentation before the line's `*` (compat-mode only). */
  get initial(): string | null;
  /** Line ending of the tag's first line (compat-mode only). */
  get lineEnd(): string | null;
  toJSON(): RemoteJsonObject;
  [inspectSymbol](): object;
}
declare const RemoteJsdocTagName_base: LazyNodeConstructor;
/** `JsdocTagName` (Kind 0x04) — the `@name` token text. */
declare class RemoteJsdocTagName extends RemoteJsdocTagName_base {}
interface RemoteJsdocTagName {
  readonly value: string;
}
declare const RemoteJsdocTagNameValue_base: LazyNodeConstructor;
/** `JsdocTagNameValue` (Kind 0x05) — value after the type in `@param`. */
declare class RemoteJsdocTagNameValue extends RemoteJsdocTagNameValue_base {}
interface RemoteJsdocTagNameValue {
  readonly raw: string;
}
declare const RemoteJsdocTypeSource_base: LazyNodeConstructor;
/** `JsdocTypeSource` (Kind 0x06) — raw `{...}` text inside a tag. */
declare class RemoteJsdocTypeSource extends RemoteJsdocTypeSource_base {}
interface RemoteJsdocTypeSource {
  readonly raw: string;
}
declare const RemoteJsdocRawTagBody_base: LazyNodeConstructor;
/** `JsdocRawTagBody` (Kind 0x0B) — raw text body fallback. */
declare class RemoteJsdocRawTagBody extends RemoteJsdocRawTagBody_base {}
interface RemoteJsdocRawTagBody {
  readonly raw: string;
}
declare const RemoteJsdocNamepathSource_base: LazyNodeConstructor;
/** `JsdocNamepathSource` (Kind 0x0D) — namepath token. */
declare class RemoteJsdocNamepathSource extends RemoteJsdocNamepathSource_base {}
interface RemoteJsdocNamepathSource {
  readonly raw: string;
}
declare const RemoteJsdocIdentifier_base: LazyNodeConstructor;
/** `JsdocIdentifier` (Kind 0x0E) — bare identifier. */
declare class RemoteJsdocIdentifier extends RemoteJsdocIdentifier_base {}
interface RemoteJsdocIdentifier {
  readonly name: string;
}
declare const RemoteJsdocText_base: LazyNodeConstructor;
/** `JsdocText` (Kind 0x0F) — raw text. */
declare class RemoteJsdocText extends RemoteJsdocText_base {}
interface RemoteJsdocText {
  readonly value: string;
}
/**
 * `JsdocTypeLine` (Kind 0x07).
 */
declare class RemoteJsdocTypeLine implements LazyNode {
  #private;
  readonly type = "JsdocTypeLine";
  constructor(view: DataView, byteIndex: number, index: number, rootIndex: number, parent: LazyNode | null, sourceFile: RemoteSourceFileLike);
  get range(): readonly [number, number];
  get parent(): LazyNode | null;
  /** Raw `{...}` line content. Basic mode reads the String payload;
   * compat mode reads byte 0-5 of the Extended Data record. */
  get rawType(): string;
  /** Source-preserving `*` line-prefix (compat-mode only; `null` otherwise). */
  get delimiter(): string | null;
  /** Source-preserving space after `*` (compat-mode only). */
  get postDelimiter(): string | null;
  /** Indentation before the leading `*` (compat-mode only). */
  get initial(): string | null;
  toJSON(): RemoteJsonObject;
  [inspectSymbol](): object;
}
/**
 * `JsdocInlineTag` (Kind 0x08) — e.g. `{@link Foo}`.
 */
declare class RemoteJsdocInlineTag implements LazyNode {
  #private;
  readonly type = "JsdocInlineTag";
  constructor(view: DataView, byteIndex: number, index: number, rootIndex: number, parent: LazyNode | null, sourceFile: RemoteSourceFileLike);
  get range(): readonly [number, number];
  get parent(): LazyNode | null;
  /** Inline tag format string. In compat mode the `'unknown'` variant is
   * mapped to `'plain'` to mirror jsdoccomment's behavior. */
  get format(): InlineTagFormat;
  /** Optional name path or URL portion. */
  get namepathOrURL(): string | null;
  /** Optional display text portion. */
  get text(): string | null;
  /** Raw body text fallback. */
  get rawBody(): string | null;
  toJSON(): RemoteJsonObject;
  [inspectSymbol](): object;
}
/**
 * `JsdocGenericTagBody` (Kind 0x09).
 */
declare class RemoteJsdocGenericTagBody implements LazyNode {
  #private;
  readonly type = "JsdocGenericTagBody";
  constructor(view: DataView, byteIndex: number, index: number, rootIndex: number, parent: LazyNode | null, sourceFile: RemoteSourceFileLike);
  get range(): readonly [number, number];
  get parent(): LazyNode | null;
  /** `true` when the tag separator was `-`. */
  get hasDashSeparator(): boolean;
  /** Description text after the dash separator. */
  get description(): string | null;
  toJSON(): RemoteJsonObject;
  [inspectSymbol](): object;
}
/**
 * `JsdocBorrowsTagBody` (Kind 0x0A) — Children type with `source` + `target`
 * children. The child accessors will be filled in once the parser starts
 * emitting them; for now the class exposes the standard range/parent/toJSON
 * surface.
 */
declare class RemoteJsdocBorrowsTagBody implements LazyNode {
  #private;
  readonly type = "JsdocBorrowsTagBody";
  constructor(view: DataView, byteIndex: number, index: number, rootIndex: number, parent: LazyNode | null, sourceFile: RemoteSourceFileLike);
  get range(): readonly [number, number];
  get parent(): LazyNode | null;
  toJSON(): RemoteJsonObject;
  [inspectSymbol](): object;
}
/**
 * `JsdocParameterName` (Kind 0x0C) — `JsdocTagValue::Parameter` variant.
 */
declare class RemoteJsdocParameterName implements LazyNode {
  #private;
  readonly type = "JsdocParameterName";
  constructor(view: DataView, byteIndex: number, index: number, rootIndex: number, parent: LazyNode | null, sourceFile: RemoteSourceFileLike);
  get range(): readonly [number, number];
  get parent(): LazyNode | null;
  /** `true` when the parameter was wrapped in `[id]` brackets. */
  get optional(): boolean;
  /** Path text. */
  get path(): string;
  /** Default value parsed from `[id=foo]` syntax. */
  get defaultValue(): string | null;
  toJSON(): RemoteJsonObject;
  [inspectSymbol](): object;
}
//#endregion
//#region src/internal/nodes/type-nodes.d.ts
declare const RemoteTypeName_base: LazyNodeConstructor;
declare class RemoteTypeName extends RemoteTypeName_base {}
declare const RemoteTypeNumber_base: LazyNodeConstructor;
declare class RemoteTypeNumber extends RemoteTypeNumber_base {}
declare const RemoteTypeStringValue_base: LazyNodeConstructor;
declare class RemoteTypeStringValue extends RemoteTypeStringValue_base {}
declare const RemoteTypeProperty_base: LazyNodeConstructor;
declare class RemoteTypeProperty extends RemoteTypeProperty_base {}
declare const RemoteTypeSpecialNamePath_base: LazyNodeConstructor;
declare class RemoteTypeSpecialNamePath extends RemoteTypeSpecialNamePath_base {}
declare const RemoteTypeUnion_base: LazyNodeConstructor;
declare class RemoteTypeUnion extends RemoteTypeUnion_base {}
declare const RemoteTypeIntersection_base: LazyNodeConstructor;
declare class RemoteTypeIntersection extends RemoteTypeIntersection_base {}
declare class RemoteTypeObject implements LazyNode {
  readonly type = "TypeObject";
  readonly _internal: RemoteInternal;
  constructor(view: DataView, byteIndex: number, index: number, rootIndex: number, parent: LazyNode | null, sourceFile: RemoteSourceFileLike);
  get range(): readonly [number, number];
  get parent(): LazyNode | null;
  get elements(): readonly LazyNode[];
  /** `bits[0:2]` of Common Data — field separator style. */
  get separator(): number;
  toJSON(): RemoteJsonObject;
  [inspectSymbol](): object;
}
declare const RemoteTypeTuple_base: LazyNodeConstructor;
declare class RemoteTypeTuple extends RemoteTypeTuple_base {}
declare const RemoteTypeTypeParameter_base: LazyNodeConstructor;
declare class RemoteTypeTypeParameter extends RemoteTypeTypeParameter_base {}
declare const RemoteTypeParameterList_base: LazyNodeConstructor;
declare class RemoteTypeParameterList extends RemoteTypeParameterList_base {}
declare const RemoteTypeParenthesis_base: LazyNodeConstructor;
declare class RemoteTypeParenthesis extends RemoteTypeParenthesis_base {}
declare const RemoteTypeInfer_base: LazyNodeConstructor;
declare class RemoteTypeInfer extends RemoteTypeInfer_base {}
declare const RemoteTypeKeyOf_base: LazyNodeConstructor;
declare class RemoteTypeKeyOf extends RemoteTypeKeyOf_base {}
declare const RemoteTypeTypeOf_base: LazyNodeConstructor;
declare class RemoteTypeTypeOf extends RemoteTypeTypeOf_base {}
declare const RemoteTypeImport_base: LazyNodeConstructor;
declare class RemoteTypeImport extends RemoteTypeImport_base {}
declare const RemoteTypeAssertsPlain_base: LazyNodeConstructor;
declare class RemoteTypeAssertsPlain extends RemoteTypeAssertsPlain_base {}
declare const RemoteTypeReadonlyArray_base: LazyNodeConstructor;
declare class RemoteTypeReadonlyArray extends RemoteTypeReadonlyArray_base {}
declare const RemoteTypeIndexedAccessIndex_base: LazyNodeConstructor;
declare class RemoteTypeIndexedAccessIndex extends RemoteTypeIndexedAccessIndex_base {}
declare const RemoteTypeReadonlyProperty_base: LazyNodeConstructor;
declare class RemoteTypeReadonlyProperty extends RemoteTypeReadonlyProperty_base {}
declare const RemoteTypeNullable_base: LazyNodeConstructor;
declare class RemoteTypeNullable extends RemoteTypeNullable_base {}
declare const RemoteTypeNotNullable_base: LazyNodeConstructor;
declare class RemoteTypeNotNullable extends RemoteTypeNotNullable_base {}
declare const RemoteTypeOptional_base: LazyNodeConstructor;
declare class RemoteTypeOptional extends RemoteTypeOptional_base {}
declare const RemoteTypeVariadic_base: LazyNodeConstructor;
/** `TypeVariadic` — modifier + extra `square_brackets` flag. */
declare class RemoteTypeVariadic extends RemoteTypeVariadic_base {}
declare const RemoteTypePredicate_base: LazyNodeConstructor;
declare class RemoteTypePredicate extends RemoteTypePredicate_base {}
declare const RemoteTypeAsserts_base: LazyNodeConstructor;
declare class RemoteTypeAsserts extends RemoteTypeAsserts_base {}
declare const RemoteTypeNamePath_base: LazyNodeConstructor;
declare class RemoteTypeNamePath extends RemoteTypeNamePath_base {}
/** `TypeGeneric` — `left` + `elements` NodeList + brackets/dot flags. */
declare class RemoteTypeGeneric implements LazyNode {
  readonly type = "TypeGeneric";
  readonly _internal: RemoteInternal;
  constructor(view: DataView, byteIndex: number, index: number, rootIndex: number, parent: LazyNode | null, sourceFile: RemoteSourceFileLike);
  get range(): readonly [number, number];
  get parent(): LazyNode | null;
  get brackets(): number;
  get dot(): boolean;
  get left(): LazyNode | null;
  get elements(): readonly LazyNode[];
  toJSON(): RemoteJsonObject;
  [inspectSymbol](): object;
}
/** `TypeFunction` — parameters + return + type_parameters. */
declare class RemoteTypeFunction implements LazyNode {
  readonly type = "TypeFunction";
  readonly _internal: RemoteInternal;
  constructor(view: DataView, byteIndex: number, index: number, rootIndex: number, parent: LazyNode | null, sourceFile: RemoteSourceFileLike);
  get range(): readonly [number, number];
  get parent(): LazyNode | null;
  get constructor_(): boolean;
  get arrow(): boolean;
  get parenthesis(): boolean;
  get parameters(): LazyNode | null;
  get returnType(): LazyNode | null;
  get typeParameters(): LazyNode | null;
  toJSON(): RemoteJsonObject;
  [inspectSymbol](): object;
}
/** `TypeConditional` — check / extends / true / false branches. */
declare class RemoteTypeConditional implements LazyNode {
  readonly type = "TypeConditional";
  readonly _internal: RemoteInternal;
  constructor(view: DataView, byteIndex: number, index: number, rootIndex: number, parent: LazyNode | null, sourceFile: RemoteSourceFileLike);
  get range(): readonly [number, number];
  get parent(): LazyNode | null;
  get checkType(): LazyNode | null;
  get extendsType(): LazyNode | null;
  get trueType(): LazyNode | null;
  get falseType(): LazyNode | null;
  toJSON(): RemoteJsonObject;
  [inspectSymbol](): object;
}
/** `TypeObjectField` — key + right + flags. */
declare class RemoteTypeObjectField implements LazyNode {
  readonly type = "TypeObjectField";
  readonly _internal: RemoteInternal;
  constructor(view: DataView, byteIndex: number, index: number, rootIndex: number, parent: LazyNode | null, sourceFile: RemoteSourceFileLike);
  get range(): readonly [number, number];
  get parent(): LazyNode | null;
  get optional(): boolean;
  get readonly(): boolean;
  get quote(): number;
  get key(): LazyNode | null;
  get right(): LazyNode | null;
  toJSON(): RemoteJsonObject;
  [inspectSymbol](): object;
}
/** `TypeJsdocObjectField` — key + right (no flags). */
declare class RemoteTypeJsdocObjectField implements LazyNode {
  readonly type = "TypeJsdocObjectField";
  readonly _internal: RemoteInternal;
  constructor(view: DataView, byteIndex: number, index: number, rootIndex: number, parent: LazyNode | null, sourceFile: RemoteSourceFileLike);
  get range(): readonly [number, number];
  get parent(): LazyNode | null;
  get key(): LazyNode | null;
  get right(): LazyNode | null;
  toJSON(): RemoteJsonObject;
  [inspectSymbol](): object;
}
declare const RemoteTypeCallSignature_base: LazyNodeConstructor;
declare class RemoteTypeCallSignature extends RemoteTypeCallSignature_base {}
declare const RemoteTypeConstructorSignature_base: LazyNodeConstructor;
declare class RemoteTypeConstructorSignature extends RemoteTypeConstructorSignature_base {}
/** `TypeKeyValue` — key string in Extended Data + first child as `right`. */
declare class RemoteTypeKeyValue implements LazyNode {
  readonly type = "TypeKeyValue";
  readonly _internal: RemoteInternal;
  constructor(view: DataView, byteIndex: number, index: number, rootIndex: number, parent: LazyNode | null, sourceFile: RemoteSourceFileLike);
  get range(): readonly [number, number];
  get parent(): LazyNode | null;
  get optional(): boolean;
  get variadic(): boolean;
  get key(): string;
  get right(): LazyNode | null;
  toJSON(): RemoteJsonObject;
  [inspectSymbol](): object;
}
declare const RemoteTypeIndexSignature_base: LazyNodeConstructor;
declare class RemoteTypeIndexSignature extends RemoteTypeIndexSignature_base {}
declare const RemoteTypeMappedType_base: LazyNodeConstructor;
declare class RemoteTypeMappedType extends RemoteTypeMappedType_base {}
/** `TypeMethodSignature` — name string in Extended Data + Common Data flags. */
declare class RemoteTypeMethodSignature implements LazyNode {
  readonly type = "TypeMethodSignature";
  readonly _internal: RemoteInternal;
  constructor(view: DataView, byteIndex: number, index: number, rootIndex: number, parent: LazyNode | null, sourceFile: RemoteSourceFileLike);
  get range(): readonly [number, number];
  get parent(): LazyNode | null;
  get quote(): number;
  get hasParameters(): boolean;
  get hasTypeParameters(): boolean;
  get name(): string;
  toJSON(): RemoteJsonObject;
  [inspectSymbol](): object;
}
/** `TypeTemplateLiteral` — literal-segment array in Extended Data. */
declare class RemoteTypeTemplateLiteral implements LazyNode {
  readonly type = "TypeTemplateLiteral";
  readonly _internal: RemoteInternal;
  constructor(view: DataView, byteIndex: number, index: number, rootIndex: number, parent: LazyNode | null, sourceFile: RemoteSourceFileLike);
  get range(): readonly [number, number];
  get parent(): LazyNode | null;
  /** Number of literal segments stored at byte 0-1 of Extended Data. */
  get literalCount(): number;
  /** Resolve the n-th literal segment. */
  literal(index: number): string;
  /** All literal segments as an array. */
  get literals(): string[];
  toJSON(): RemoteJsonObject;
  [inspectSymbol](): object;
}
/** `TypeSymbol` — `Symbol(...)` callee value + optional element. */
declare class RemoteTypeSymbol implements LazyNode {
  readonly type = "TypeSymbol";
  readonly _internal: RemoteInternal;
  constructor(view: DataView, byteIndex: number, index: number, rootIndex: number, parent: LazyNode | null, sourceFile: RemoteSourceFileLike);
  get range(): readonly [number, number];
  get parent(): LazyNode | null;
  get hasElement(): boolean;
  get value(): string;
  get element(): LazyNode | null;
  toJSON(): RemoteJsonObject;
  [inspectSymbol](): object;
}
declare const RemoteTypeNull_base: LazyNodeConstructor;
declare class RemoteTypeNull extends RemoteTypeNull_base {}
declare const RemoteTypeUndefined_base: LazyNodeConstructor;
declare class RemoteTypeUndefined extends RemoteTypeUndefined_base {}
declare const RemoteTypeAny_base: LazyNodeConstructor;
declare class RemoteTypeAny extends RemoteTypeAny_base {}
declare const RemoteTypeUnknown_base: LazyNodeConstructor;
declare class RemoteTypeUnknown extends RemoteTypeUnknown_base {}
declare const RemoteTypeUniqueSymbol_base: LazyNodeConstructor;
declare class RemoteTypeUniqueSymbol extends RemoteTypeUniqueSymbol_base {}
//#endregion
//#region src/internal/nodes/node-list-node.d.ts
declare class RemoteNodeListNode implements LazyNode {
  readonly type = "NodeList";
  private readonly _internal;
  constructor(view: DataView, byteIndex: number, index: number, rootIndex: number, parent: LazyNode | null, sourceFile: RemoteSourceFileLike);
  get range(): readonly [number, number];
  get parent(): LazyNode | null;
  /** Number of elements (stored in the 30-bit Children payload). */
  get elementCount(): number;
  /** Walk and return the wrapper's children as a plain Array. */
  get children(): LazyNode[];
  toJSON(): RemoteJsonObject;
  [inspectSymbol](): object;
}
//#endregion
//#region src/index.d.ts
/**
 * Visitor keys for every Remote* node kind (60 = 15 Comment AST + 45 TypeNode).
 *
 * Each entry maps a node `type` name to the **traversable child property
 * names** in canonical visit order. Mirrors the jsdoccomment / ESLint
 * `visitorKeys` convention — frameworks that depend on it (`estraverse`,
 * `eslint-visitor-keys`, etc.) can spread this object directly into their
 * own key map.
 *
 * **Differences from jsdoccomment**: ox-jsdoc emits `JsdocTag.tag` /
 * `rawType` / `name` / `body` as actual child nodes (`RemoteJsdocTagName`,
 * `RemoteJsdocTypeSource`, `RemoteJsdocTagNameValue`, `RemoteJsdocTagBody`)
 * instead of flattening them to strings, so those property names appear in
 * `JsdocTag`'s key list. Strict-jsdoccomment consumers can filter the list
 * down to `['parsedType', 'typeLines', 'descriptionLines', 'inlineTags']`.
 *
 * **Reserved kinds** (`JsdocBorrowsTagBody`, `JsdocRawTagBody`) are listed
 * for future use; the parser does not currently emit them
 * (see `design/007-binary-ast/ast-nodes.md` "Reserved Kinds").
 */
declare const jsdocVisitorKeys: Readonly<{
  JsdocBlock: string[];
  JsdocDescriptionLine: never[];
  JsdocTag: string[];
  JsdocTagName: never[];
  JsdocTagNameValue: never[];
  JsdocTypeSource: never[];
  JsdocTypeLine: never[];
  JsdocInlineTag: never[];
  JsdocGenericTagBody: string[];
  JsdocBorrowsTagBody: string[];
  JsdocRawTagBody: never[];
  JsdocParameterName: never[];
  JsdocNamepathSource: never[];
  JsdocIdentifier: never[];
  JsdocText: never[];
  TypeName: never[];
  TypeNumber: never[];
  TypeStringValue: never[];
  TypeProperty: never[];
  TypeSpecialNamePath: never[];
  TypeNull: never[];
  TypeUndefined: never[];
  TypeAny: never[];
  TypeUnknown: never[];
  TypeUniqueSymbol: never[];
  TypeUnion: string[];
  TypeIntersection: string[];
  TypeObject: string[];
  TypeTuple: string[];
  TypeTypeParameter: string[];
  TypeParameterList: string[];
  TypeParenthesis: string[];
  TypeInfer: string[];
  TypeKeyOf: string[];
  TypeTypeOf: string[];
  TypeImport: string[];
  TypeAssertsPlain: string[];
  TypeReadonlyArray: string[];
  TypeIndexedAccessIndex: string[];
  TypeReadonlyProperty: string[];
  TypeNullable: string[];
  TypeNotNullable: string[];
  TypeOptional: string[];
  TypeVariadic: string[];
  TypePredicate: string[];
  TypeAsserts: string[];
  TypeNamePath: string[];
  TypeGeneric: string[];
  TypeFunction: string[];
  TypeConditional: string[];
  TypeObjectField: string[];
  TypeJsdocObjectField: string[];
  TypeKeyValue: string[];
  TypeIndexSignature: string[];
  TypeMappedType: string[];
  TypeMethodSignature: string[];
  TypeCallSignature: string[];
  TypeConstructorSignature: string[];
  TypeTemplateLiteral: never[];
  TypeSymbol: string[];
}>;
/**
 * Recursively convert a Remote* lazy node into a plain JSON object.
 * Handy for browser DevTools (where `Symbol.for('nodejs.util.inspect.custom')`
 * has no effect) and for general logging.
 */
declare function toPlainObject(node: unknown): unknown;
//#endregion
export { EMPTY_NODE_LIST, RemoteJsdocBlock, RemoteJsdocBorrowsTagBody, RemoteJsdocDescriptionLine, RemoteJsdocGenericTagBody, RemoteJsdocIdentifier, RemoteJsdocInlineTag, RemoteJsdocNamepathSource, RemoteJsdocParameterName, RemoteJsdocRawTagBody, RemoteJsdocTag, RemoteJsdocTagName, RemoteJsdocTagNameValue, RemoteJsdocText, RemoteJsdocTypeLine, RemoteJsdocTypeSource, RemoteNodeList, RemoteNodeListNode, RemoteSourceFile, RemoteTypeAny, RemoteTypeAsserts, RemoteTypeAssertsPlain, RemoteTypeCallSignature, RemoteTypeConditional, RemoteTypeConstructorSignature, RemoteTypeFunction, RemoteTypeGeneric, RemoteTypeImport, RemoteTypeIndexSignature, RemoteTypeIndexedAccessIndex, RemoteTypeInfer, RemoteTypeIntersection, RemoteTypeJsdocObjectField, RemoteTypeKeyOf, RemoteTypeKeyValue, RemoteTypeMappedType, RemoteTypeMethodSignature, RemoteTypeName, RemoteTypeNamePath, RemoteTypeNotNullable, RemoteTypeNull, RemoteTypeNullable, RemoteTypeNumber, RemoteTypeObject, RemoteTypeObjectField, RemoteTypeOptional, RemoteTypeParameterList, RemoteTypeParenthesis, RemoteTypePredicate, RemoteTypeProperty, RemoteTypeReadonlyArray, RemoteTypeReadonlyProperty, RemoteTypeSpecialNamePath, RemoteTypeStringValue, RemoteTypeSymbol, RemoteTypeTemplateLiteral, RemoteTypeTuple, RemoteTypeTypeOf, RemoteTypeTypeParameter, RemoteTypeUndefined, RemoteTypeUnion, RemoteTypeUniqueSymbol, RemoteTypeUnknown, RemoteTypeVariadic, jsdocVisitorKeys, toPlainObject };