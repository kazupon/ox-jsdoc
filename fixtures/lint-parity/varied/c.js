// a.js and c.js have the same content, and b.js and d.js have the same
// content. The comments are on the same lines in all four files, but the
// types and the tag contents differ between a.js and b.js. a.js defines a
// typedef that b.js uses without defining it.
//
// Oxlint shares one SourceCode object between files. A cache of parsed
// comments that outlives a file would give the next file the wrong blocks.

/**
 * @param x The value.
 * @returns The value.
 */
export function foo(x) {
  return x
}

/**
 * @abstract
 * @param y
 */
export function bar(y) {
  return y
}

/**
 * @typedef {object} OnlyInA
 * @property {number} value The value.
 */

/**
 * @param {OnlyInA} item The item.
 * @returns {number} The value.
 */
export function useOnlyInA(item) {
  return item.value
}
