/**
 * `RemoteNodeList` — Array-compatible view over a NodeList wrapper child.
 *
 * Mirrors tsgo's RemoteNodeList: a real `Array` subclass populated with
 * lazy class instances pulled out of the NodeList wrapper's `next_sibling`
 * chain. Empty arrays share `EMPTY_NODE_LIST` to avoid per-call allocation.
 *
 * @author kazuya kawaguchi (a.k.a. kazupon)
 * @license MIT
 */

// @ts-check

import {
  childIndexAtVisitorIndex,
  childrenBitmaskPayloadOf,
  extOffsetOf,
  firstChildIndex,
  readNextSibling,
  thisNode
} from './helpers.js'

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
 * Build a `RemoteNodeList` containing the children of the NodeList wrapper
 * at the given visitor index of an Extended-type parent (e.g. JsdocBlock,
 * JsdocTag).
 *
 * Returns `EMPTY_NODE_LIST` when the slot is unset or the wrapper has no
 * children — both encode "empty array" by spec (encoding.md).
 *
 * @param {import('./helpers.js').RemoteInternal} internal
 * @param {number} visitorIndex
 * @returns {RemoteNodeList}
 */
export function nodeListAtVisitorIndexExtended(internal, visitorIndex) {
  const bitmask = internal.view.getUint8(extOffsetOf(internal))
  const childIdx = childIndexAtVisitorIndex(internal, bitmask, visitorIndex)
  if (childIdx === 0) {
    return EMPTY_NODE_LIST
  }
  return collectNodeListChildren(internal, childIdx)
}

/**
 * Same as `nodeListAtVisitorIndexExtended` but for Children-type parents
 * (e.g. TypeUnion) where the bitmask lives in the 30-bit Node Data payload.
 *
 * @param {import('./helpers.js').RemoteInternal} internal
 * @param {number} visitorIndex
 * @returns {RemoteNodeList}
 */
export function nodeListAtVisitorIndexChildren(internal, visitorIndex) {
  const bitmask = childrenBitmaskPayloadOf(internal) & 0xff
  const childIdx = childIndexAtVisitorIndex(internal, bitmask, visitorIndex)
  if (childIdx === 0) {
    return EMPTY_NODE_LIST
  }
  return collectNodeListChildren(internal, childIdx)
}

/**
 * Walk the NodeList wrapper at `nodeListIndex` and collect its children
 * into a `RemoteNodeList`. The parent of every collected child is `internal`.
 *
 * @param {import('./helpers.js').RemoteInternal} parentInternal
 * @param {number} nodeListIndex Index of the NodeList wrapper.
 * @returns {RemoteNodeList}
 */
function collectNodeListChildren(parentInternal, nodeListIndex) {
  const { sourceFile, rootIndex } = parentInternal
  const head = firstChildIndex(sourceFile, nodeListIndex)
  if (head === 0) {
    return EMPTY_NODE_LIST
  }

  const list = new RemoteNodeList()
  const parent = thisNode(parentInternal)
  let cursor = head
  while (cursor !== 0) {
    list.push(sourceFile.getNode(cursor, parent, rootIndex))
    cursor = readNextSibling(sourceFile, cursor)
  }
  return list
}
