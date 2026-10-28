// a.js, b.js and c.js have the same content.
//
// Oxlint reuses comment objects between files. A rule that remembers
// comments from an earlier file would skip the same comments here.

/**
 * Adds two numbers.
 * @param {number} a The first number.
 * @param b The second number.
 * @returns {number}
 */
export function add(a, b) {
  return a + b
}

/**
 * @abstract something
 * @param {string} name
 * @param {string} extra Not in the signature.
 */
export function greet(name) {
  return `hello ${name}`
}

/**
 * @access foo
 * @unknowntag
 * @param {strin g} value A broken type.
 * @param {Foo} other An undefined type.
 * @returns {void}
 */
export function broken(value, other) {
  console.log(value, other)
}

/**
 *  Misaligned asterisk.
  * @param {number} x The value.
 */
export function misaligned(x) {
  return x
}

/** @type {number} */
export const typed = 1

/**
 * A comment that is not attached to any node.
 * @todo
 */

/**
 * @typedef {object} Shape
 * @property {number} width
 * @property height The height.
 * @property {number}
 */

/**
 * Creates a shape.
 * @param {number} w
 * @return {Shape} The shape.
 */
export function createShape(w) {
  return { width: w, height: w }
}

/**
 * @class
 * @implements {Shape}
 */
export function NotAClass() {}

export class Box {
  /**
   * @param {number} size
   */
  constructor(size) {
    this.size = size
  }

  /**
   * Returns the size.
   */
  getSize() {
    return this.size
  }
}

/**
 * Yields values.
 * @yields
 */
export function* gen() {
  yield 1
}

/**
 * @param {number} [x=1] The value.
 * @returns {Promise<number>} The value.
 */
export async function withDefault(x = 1) {
  return x
}

/**
 * @param {any} value The value.
 * @param {Function} callback The callback.
 * @param {*} other Another value.
 * @param {{}} empty An empty object type.
 */
export function loose(value, callback, other, empty) {
  callback(value, other, empty)
}

/**
 ** Multiple asterisks.
 * @param {number} n The number.
 * @returns {number} The number.
 */
export function multi(n) {
  return n
}

/** @param {number} n The number. */
export function singleLine(n) {
  return n
}

/**
 * Throws.
 * @throws
 * @param {string} message The message.
 */
export function fail(message) {
  throw new Error(message)
}

/**
 * Escapes {@link
 * @param {string} s The string.
 * @returns {string} The string.
 */
export function escape(s) {
  return s
}

export const arrow = (a, b) => a + b

/**
 * @template T
 * @param {T} value The value.
 * @returns {T} The value.
 */
export function identity(value) {
  return value
}

/**
 * @see
 * @since
 * @version
 * @license
 */
export function emptyTags() {}

/**
 * Has a tag with a missing description.
 * @param {number} count
 * @returns {number}
 */
export function count(count) {
  return count
}

/**
 * @param {String} s A wrongly cased type.
 * @returns {Object} An object.
 */
export function wrongCase(s) {
  return { s }
}

/**
 * @typedef {object} Duplicated
 * @property {number} size The size.
 * @property {number} size The size again.
 */

/**
 * @implements {Shape}
 */
export function notAClassEither() {}

/** Text on the first line.
 * And more text.
 */
export function zeroLine() {}

/* @param {number} x A tag in a block that is not JSDoc. */
export function badBlock(x) {
  return x
}

/**
 */
export function blank() {}

/**
 *
 * @param {number} n The number.
 */
export function blankDescription(n) {
  return n
}

/**
 * Missing asterisk.
   @param {number} n The number.
 */
export function noAsterisk(n) {
  return n
}

export function undocumented() {}

/**
 * Has a next tag without a type.
 * @next
 * @yields {number} A number.
 */
export function* nextWithoutType() {
  yield 1
}

/**
 * Has a param without a name.
 * @param {string}
 */
export function unnamed(s) {
  return s
}

/**
 * @typedef {object} NoProperties
 */

/**
 * Claims to return.
 * @returns {number} A number.
 */
export function noReturn() {}

/**
 * Throws without saying so.
 */
export function throwsSilently() {
  throw new Error('silent')
}

/**
 * Yields without saying so.
 */
export function* yieldsSilently() {
  yield 1
}

/**
 * Claims to yield.
 * @yields {number} A number.
 */
export function* noYield() {}

/**
 * Has lines between tags.
 * @param {number} a The first.
 *
 * @param {number} b The second.
 */
export function linesBetween(a, b) {
  return a + b
}
export const before = 1
/**
 * A block right after code.
 */
export const after = 2

/**
 * @template T, U
 * @typedef {object} Pair
 * @property {T} first The first.
 */

/**
 * Has a type variable without a template.
 * @typedef {object} Generic
 * @property {V} value The value.
 */

/**
 * Supports shorthands such as @yearly in the description.
 * @param {number}  aligned  The value.
 * @returns {number} The value.
 */
export function shorthand(aligned) {
  return aligned
}
