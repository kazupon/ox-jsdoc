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

// @ts-check

import { extOffsetOf, readNextSibling, thisNode } from './helpers.js'

/**
 * `Array` subclass returned by every "node list" getter. Inheriting from
 * `Array` gives us `length` / `map` / `filter` / `forEach` etc. for free;
 * indexed access (`list[i]`) returns lazy class instances built up front.
 */
export class RemoteNodeList extends Array {}

/**
 * Empty singleton — every "no children" getter returns this so callers can
 * branch on `length === 0` without allocating.
 */
export const EMPTY_NODE_LIST = new RemoteNodeList()

/**
 * Build a `RemoteNodeList` from the per-list metadata slot at byte offset
 * `slotOffset` inside the parent's Extended Data block. Reads
 * `(head_index: u32, count: u16)` and walks `count` siblings starting from
 * `head_index`.
 *
 * Mirrors `decoder::helpers::read_list_metadata` + `NodeListIter::new` on
 * the Rust side.
 *
 * @param {import('./helpers.js').RemoteInternal} internal
 * @param {number} slotOffset Per-Kind byte offset of the list metadata.
 * @returns {RemoteNodeList}
 */
export function nodeListAtSlotExtended(internal, slotOffset) {
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
 *
 * @param {import('./helpers.js').RemoteInternal} parentInternal
 * @param {number} headIndex   First node index in the list.
 * @param {number} count       Number of elements to walk.
 * @returns {RemoteNodeList}
 */
function collectNodeListChildren(parentInternal, headIndex, count) {
  const { sourceFile, rootIndex } = parentInternal
  const list = new RemoteNodeList()
  const parent = thisNode(parentInternal)
  let cursor = headIndex
  for (let i = 0; i < count && cursor !== 0; i++) {
    list.push(sourceFile.getNode(cursor, parent, rootIndex))
    cursor = readNextSibling(sourceFile, cursor)
  }
  return list
}
