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
export type RemoteJsonValue = string | number | boolean | null | RemoteJsonObject | RemoteJsonArray
export interface RemoteJsonObject {
  [key: string]: RemoteJsonValue | undefined
}
export type RemoteJsonArray = ReadonlyArray<RemoteJsonValue>

/**
 * Marker shape implemented by every Remote* lazy class — kept loose so
 * helpers can pass instances around as `parent` / list children without
 * knowing the per-Kind discriminant.
 */
export interface LazyNode {
  readonly type: string
  readonly range: readonly [number, number]
  readonly parent: LazyNode | null
  toJSON(): RemoteJsonObject
}

/**
 * Public-facing surface of `RemoteSourceFile` consumed by helpers and
 * lazy node classes. Mirrors the runtime API but stays an interface so
 * it can be referenced before the class definition is loaded (avoids
 * circular type references between `source-file.ts` and `helpers.ts`).
 */
export interface RemoteSourceFileLike {
  readonly view: DataView
  readonly uint32View: Uint32Array
  readonly compatMode: boolean
  readonly emptyStringForNull: boolean
  readonly extendedDataOffset: number
  readonly nodesOffset: number
  readonly nodeCount: number
  readonly rootCount: number
  getString(idx: number): string | null
  getStringByField(offset: number, length: number): string | null
  getStringByOffsetAndLength(offset: number, length: number): string
  getRootBaseOffset(rootIndex: number): number
  getRootSourceOffsetInData(rootIndex: number): number
  getRootSourceText(rootIndex: number): string
  sliceSourceText(rootIndex: number, start: number, end: number): string | null
  getNode(nodeIndex: number, parent: LazyNode | null, rootIndex?: number): LazyNode | null
}

/**
 * Shared `_internal` shape held by every Remote* lazy class. All helpers
 * in `helpers.ts` accept this type and read from it without knowing which
 * concrete class instance owns it.
 */
export interface RemoteInternal {
  readonly view: DataView
  /** Byte offset of this node record (= nodesOffset + index * 24). */
  readonly byteIndex: number
  /** Node index (0 = sentinel). */
  readonly index: number
  /** Index of the root this node belongs to (used for absolute range). */
  readonly rootIndex: number
  /** The parent Remote* instance (null for roots). */
  readonly parent: LazyNode | null
  readonly sourceFile: RemoteSourceFileLike
}

/**
 * Constructor signature shared by every lazy class instantiated through
 * `decodeKindToClass`. Kept as a tuple so the kind dispatch table can
 * type-check `new Class(...)` calls.
 */
export type LazyNodeConstructor = new (
  view: DataView,
  byteIndex: number,
  index: number,
  rootIndex: number,
  parent: LazyNode | null,
  sourceFile: RemoteSourceFileLike
) => LazyNode
