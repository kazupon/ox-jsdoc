/**
 * Convert ox-jsdoc's node ranges to the offsets of the playground.
 *
 * ox-jsdoc reports node ranges as UTF-8 byte offsets. oxc-parser and Monaco
 * use UTF-16 code units (JavaScript string indices). The two are equal only
 * while the text is ASCII.
 *
 * @author kazuya kawaguchi (a.k.a. kazupon)
 * @license MIT
 */

/**
 * Whether `text` has only ASCII characters, whose UTF-8 byte offsets and
 * UTF-16 offsets are the same.
 */
function isAscii(text: string): boolean {
  for (let index = 0; index < text.length; index++) {
    if (text.charCodeAt(index) > 0x7f) {
      return false
    }
  }
  return true
}

/**
 * Returns a function that maps a UTF-8 byte offset in `text` to the UTF-16
 * offset of the same position. An offset inside a character maps to the
 * start of that character.
 */
export function createUtf8ToUtf16Offset(text: string): (byteOffset: number) => number {
  if (isAscii(text)) {
    return byteOffset => byteOffset
  }

  // The UTF-8 byte offset and the UTF-16 offset of the start of each
  // character, and of the end of the text.
  const byteOffsets: number[] = []
  const utf16Offsets: number[] = []
  let byteOffset = 0
  let utf16Offset = 0
  for (const character of text) {
    byteOffsets.push(byteOffset)
    utf16Offsets.push(utf16Offset)
    const code = character.codePointAt(0) ?? 0
    // A lone surrogate is encoded as U+FFFD (3 bytes), as `TextEncoder` and
    // wasm-bindgen do.
    byteOffset += code < 0x80 ? 1 : code < 0x800 ? 2 : code < 0x10000 ? 3 : 4
    utf16Offset += character.length
  }
  byteOffsets.push(byteOffset)
  utf16Offsets.push(utf16Offset)

  return offset => {
    // The last character that starts at or before `offset`.
    let low = 0
    let high = byteOffsets.length - 1
    while (low < high) {
      const middle = (low + high + 1) >> 1
      if ((byteOffsets[middle] ?? 0) <= offset) {
        low = middle
      } else {
        high = middle - 1
      }
    }
    return utf16Offsets[low] ?? 0
  }
}

function isRange(value: unknown): value is [number, number] {
  return (
    Array.isArray(value) &&
    value.length === 2 &&
    typeof value[0] === 'number' &&
    typeof value[1] === 'number'
  )
}

/**
 * Returns `value`, a `toJSON()` result of ox-jsdoc parsed with
 * `baseOffset: 0`, with every `range` moved to UTF-16 offsets in the host
 * file: `start` (where the comment starts there) plus the UTF-16 offset in
 * `sourceText`. `value` itself is not changed.
 */
export function toUtf16Ranges<T>(value: T, sourceText: string, start: number): T {
  const toUtf16 = createUtf8ToUtf16Offset(sourceText)
  const convert = (item: unknown): unknown => {
    if (Array.isArray(item)) {
      return item.map(convert)
    }
    if (item === null || typeof item !== 'object') {
      return item
    }
    const result: Record<string, unknown> = {}
    for (const [key, child] of Object.entries(item)) {
      result[key] =
        key === 'range' && isRange(child)
          ? [start + toUtf16(child[0]), start + toUtf16(child[1])]
          : convert(child)
    }
    return result
  }
  return convert(value) as T
}
