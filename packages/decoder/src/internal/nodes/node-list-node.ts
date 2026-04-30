/**
 * `RemoteNodeListNode` — wraps the Kind 0x7F NodeList record.
 *
 * Users almost never construct one directly; the `RemoteNodeList`
 * helpers in `node-list.js` walk past the wrapper and expose its children
 * directly. This class exists so that `RemoteSourceFile.getNode` can
 * still return a stable instance for the wrapper itself (used by some
 * helpers when traversing the byte stream).
 *
 * @author kazuya kawaguchi (a.k.a. kazupon)
 * @license MIT
 */

import {
  absoluteRange,
  childrenBitmaskPayloadOf,
  firstChildIndex,
  readNextSibling,
  thisNode
} from '../helpers.ts'
import { inspectPayload, inspectSymbol } from '../inspect.ts'
import type { LazyNode, RemoteInternal, RemoteJsonObject, RemoteSourceFileLike } from '../types.ts'

export class RemoteNodeListNode implements LazyNode {
  readonly type = 'NodeList'
  private readonly _internal: RemoteInternal

  constructor(
    view: DataView,
    byteIndex: number,
    index: number,
    rootIndex: number,
    parent: LazyNode | null,
    sourceFile: RemoteSourceFileLike
  ) {
    this._internal = { view, byteIndex, index, rootIndex, parent, sourceFile }
  }
  get range(): readonly [number, number] {
    return absoluteRange(this._internal)
  }
  get parent(): LazyNode | null {
    return this._internal.parent
  }
  /** Number of elements (stored in the 30-bit Children payload). */
  get elementCount(): number {
    return childrenBitmaskPayloadOf(this._internal)
  }
  /** Walk and return the wrapper's children as a plain Array. */
  get children(): LazyNode[] {
    const out: LazyNode[] = []
    const head = firstChildIndex(this._internal.sourceFile, this._internal.index)
    let cursor = head
    const parent = thisNode(this._internal)
    while (cursor !== 0) {
      const child = this._internal.sourceFile.getNode(cursor, parent, this._internal.rootIndex)
      if (child !== null) {
        out.push(child)
      }
      cursor = readNextSibling(this._internal.sourceFile, cursor)
    }
    return out
  }
  toJSON(): RemoteJsonObject {
    return {
      type: this.type,
      range: [...this.range],
      elementCount: this.elementCount,
      children: this.children.map(n => n.toJSON())
    }
  }
  [inspectSymbol](): object {
    return inspectPayload(this.toJSON(), 'NodeList')
  }
}
