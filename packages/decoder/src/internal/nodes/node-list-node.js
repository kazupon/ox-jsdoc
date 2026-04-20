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

// @ts-check

import {
  absoluteRange,
  childrenBitmaskPayloadOf,
  firstChildIndex,
  readNextSibling,
  thisNode
} from '../helpers.js'
import { inspectPayload, inspectSymbol } from '../inspect.js'

export class RemoteNodeListNode {
  type = 'NodeList'
  _internal

  constructor(view, byteIndex, index, rootIndex, parent, sourceFile) {
    this._internal = { view, byteIndex, index, rootIndex, parent, sourceFile }
  }
  get range() {
    return absoluteRange(this._internal)
  }
  get parent() {
    return this._internal.parent
  }
  /** Number of elements (stored in the 30-bit Children payload). */
  get elementCount() {
    return childrenBitmaskPayloadOf(this._internal)
  }
  /** Walk and return the wrapper's children as a plain Array. */
  get children() {
    const out = []
    const head = firstChildIndex(this._internal.sourceFile, this._internal.index)
    let cursor = head
    const parent = thisNode(this._internal)
    while (cursor !== 0) {
      out.push(this._internal.sourceFile.getNode(cursor, parent, this._internal.rootIndex))
      cursor = readNextSibling(this._internal.sourceFile, cursor)
    }
    return out
  }
  toJSON() {
    return {
      type: this.type,
      range: this.range,
      elementCount: this.elementCount,
      children: this.children.map(n => n.toJSON())
    }
  }
  [inspectSymbol]() {
    return inspectPayload(this.toJSON(), 'NodeList')
  }
}
