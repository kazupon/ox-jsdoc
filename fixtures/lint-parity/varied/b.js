// a.js and c.js have the same content, and b.js and d.js have the same
// content. The comments are on the same lines in all four files, but the
// types and the tag contents differ between a.js and b.js. a.js defines a
// typedef that b.js uses without defining it.
//
// Oxlint shares one SourceCode object between files. A cache of parsed
// comments that outlives a file would give the next file the wrong blocks.

/**
 * @param {string} x The value.
 * @returns {string} The value.
 */
export function foo(x) {
  return x
}

/**
 * @abstract something
 * @param {number} z
 */
export function bar(y) {
  return y
}

/**
 * A block without a typedef.
 * @see OnlyInA
 */

/**
 * @param {OnlyInA} item The item.
 * @returns {number} The value.
 */
export function useOnlyInA(item) {
  return item.value
}
