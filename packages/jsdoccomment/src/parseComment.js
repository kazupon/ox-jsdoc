/* eslint-disable prefer-named-capture-group -- Temporary */
import { parse as oxJsdocBinaryParse } from 'ox-jsdoc-binary'

import { parseInlineTags } from './parseInlineTags.js'

/**
 * @param {import('comment-parser').Spec} spec
 * @returns {boolean}
 */
export const hasSeeWithLink = spec => {
  return spec.tag === 'see' && /\{@link.+?\}/v.test(spec.source[0].source)
}

export const defaultNoTypes = [
  'default',
  'defaultvalue',
  'description',
  'example',
  'file',
  'fileoverview',
  'license',
  'overview',
  'see',
  'summary'
]

export const defaultNoNames = [
  'access',
  'author',
  'default',
  'defaultvalue',
  'description',
  'example',
  'exception',
  'file',
  'fileoverview',
  'kind',
  'license',
  'overview',
  'return',
  'returns',
  'since',
  'summary',
  'throws',
  'version',
  'variation'
]

const optionalBrackets = /^\[(?<name>[^=]*)=[^\]]*\]/v

/**
 * @typedef {(spec: import('comment-parser').Spec) =>
 *   import('comment-parser').Spec} CommentParserTokenizer
 */

/**
 * @param {object} [cfg]
 * @param {string[]} [cfg.noTypes]
 * @param {string[]} [cfg.noNames]
 * @returns {CommentParserTokenizer[]}
 */
const getTokenizers = ({ noTypes = defaultNoTypes, noNames = defaultNoNames } = {}) => {
  void noTypes
  void noNames
  return []
}

/**
 * @param {string | {value: string}} commentOrNode
 * @param {string} indent
 * @returns {string}
 */
const normalizeCommentSource = (commentOrNode, indent) => {
  switch (typeof commentOrNode) {
    case 'string':
      return `${indent}${commentOrNode}`
    case 'object':
      if (commentOrNode === null) {
        throw new TypeError(`'commentOrNode' is not a string or object.`)
      }

      return `${indent}/*${commentOrNode.value}*/`

    default:
      throw new TypeError(`'commentOrNode' is not a string or object.`)
  }
}

/**
 * @param {object} tokens
 * @returns {import('comment-parser').Tokens}
 */
const normalizeTokens = tokens => {
  return {
    delimiter: tokens.delimiter ?? '',
    description: tokens.description ?? '',
    end: tokens.end ?? '',
    lineEnd: tokens.lineEnd ?? '',
    name: tokens.name ?? '',
    postDelimiter: tokens.postDelimiter ?? '',
    postName: tokens.postName ?? '',
    postTag: tokens.postTag ?? '',
    postType: tokens.postType ?? '',
    start: tokens.start ?? '',
    tag: tokens.tag ?? '',
    type: tokens.type ?? ''
  }
}

/**
 * @param {object} line
 * @returns {import('comment-parser').Line}
 */
const normalizeLine = line => {
  const tokens = normalizeTokens(line.tokens ?? {})
  return {
    number: line.number ?? 0,
    source: line.source ?? Object.values(tokens).join(''),
    tokens
  }
}

/**
 * @param {object} tag
 * @returns {import('comment-parser').Spec}
 */
const normalizeTag = tag => {
  const name = tag.name ?? ''

  return {
    tag: tag.tag ?? '',
    name,
    type: tag.rawType ?? '',
    optional: optionalBrackets.test(name),
    description: tag.description ?? '',
    inlineTags: [],
    problems: [],
    source: Array.from(tag.source ?? [], normalizeLine)
  }
}

/**
 * @param {object} block
 * @returns {import('.').JsdocBlockWithInline}
 */
const normalizeBlock = block => {
  return parseInlineTags({
    description: block.description ?? '',
    tags: Array.from(block.tags ?? [], normalizeTag),
    inlineTags: [],
    source: Array.from(block.source ?? [], normalizeLine),
    problems: []
  })
}

/**
 * Accepts a comment token or complete comment string and converts it into
 * a `comment-parser` compatible AST.
 * @param {string | {value: string}} commentOrNode
 * @param {string} [indent] Whitespace
 * @returns {import('.').JsdocBlockWithInline}
 */
const parseComment = (commentOrNode, indent = '') => {
  const sourceText = normalizeCommentSource(commentOrNode, indent)
  const { ast } = oxJsdocBinaryParse(sourceText, {
    compatMode: true,
    emptyStringForNull: true,
    preserveWhitespace: true
  })

  if (!ast) {
    throw new Error('There were no results for comment parsing')
  }

  return normalizeBlock(ast.toJSON())
}

export { getTokenizers, parseComment }
