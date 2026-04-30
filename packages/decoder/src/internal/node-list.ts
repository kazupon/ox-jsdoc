/**
 * `RemoteNodeList` — Array-compatible view over a parent's per-list metadata
 * slot.
 *
 * As of the NodeList-wrapper-elimination format change, every variable-length
 * child list is represented as an inline `(head_index: u32, count: u16)` pair
 * stored at a known per-Kind byte offset inside the parent's Extended Data
 * block. The decoder reads `head` and walks the `next_sibling` chain exactly
 * `count` times. Empty arrays share `EMPTY_NODE_LIST` to avoid per-call
 * allocation.
 *
 * @author kazuya kawaguchi (a.k.a. kazupon)
 * @license MIT
 */

import { extOffsetOf, readNextSibling, thisNode } from './helpers.ts'
import type { LazyNode, RemoteInternal } from './types.ts'

/**
 * `Array` subclass returned by every "node list" getter. Inheriting from
 * `Array` gives us `length` / `map` / `filter` / `forEach` etc. for free;
 * indexed access (`list[i]`) returns lazy class instances built up front.
 */
export class RemoteNodeList extends Array<LazyNode> {}

/**
 * Empty singleton — every "no children" getter returns this so callers can
 * branch on `length === 0` without allocating.
 */
export const EMPTY_NODE_LIST: RemoteNodeList = new RemoteNodeList()

/**
 * Build a `RemoteNodeList` from the per-list metadata slot at byte offset
 * `slotOffset` inside the parent's Extended Data block. Reads
 * `(head_index: u32, count: u16)` and walks `count` siblings starting from
 * `head_index`.
 *
 * Mirrors `decoder::helpers::read_list_metadata` + `NodeListIter::new` on
 * the Rust side.
 */
export function nodeListAtSlotExtended(
  internal: RemoteInternal,
  slotOffset: number
): RemoteNodeList {
  const ext = extOffsetOf(internal) + slotOffset
  const head = internal.view.getUint32(ext, true)
  const count = internal.view.getUint16(ext + 4, true)
  if (head === 0 || count === 0) {
    return EMPTY_NODE_LIST
  }
  return collectNodeListChildren(internal, head, count)
}

/**
 * Walk `count` siblings starting at `headIndex` and collect them into a
 * `RemoteNodeList`. The parent of every collected child is `internal`.
 */
function collectNodeListChildren(
  parentInternal: RemoteInternal,
  headIndex: number,
  count: number
): RemoteNodeList {
  const { sourceFile, rootIndex } = parentInternal
  const list = new RemoteNodeList()
  const parent = thisNode(parentInternal)
  let cursor = headIndex
  for (let i = 0; i < count && cursor !== 0; i++) {
    const child = sourceFile.getNode(cursor, parent, rootIndex)
    if (child !== null) {
      list.push(child)
    }
    cursor = readNextSibling(sourceFile, cursor)
  }
  return list
}
