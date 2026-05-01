/* eslint-disable prefer-named-capture-group -- Temporary */
import {
  parse as oxJsdocBinaryParse,
  parseBatch as oxJsdocBinaryParseBatch
} from 'ox-jsdoc-binary'

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
// eslint-disable-next-line no-div-regex -- Default assignment syntax.
const invalidDefault = /=(?!>)/v

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
 * @param {string} value
 * @returns {boolean}
 */
const isQuoted = value => {
  return value !== '' && value.startsWith('"') && value.endsWith('"')
}

/**
 * @param {import('comment-parser').Problem['code']} code
 * @param {string} message
 * @param {number} line
 * @returns {import('comment-parser').Problem}
 */
const createCriticalProblem = (code, message, line) => {
  return {
    code,
    message,
    line,
    critical: true
  }
}

/**
 * Reproduce `comment-parser`'s name tokenizer validation for the already
 * tokenized name emitted by `ox-jsdoc-binary`.
 *
 * @param {string} nameToken
 * @param {number} line
 * @returns {import('comment-parser').Problem[]}
 */
const getNameProblems = (nameToken, line) => {
  if (nameToken === '') {
    return []
  }

  const quotedGroups = nameToken.split('"')
  if (
    quotedGroups.length > 1 &&
    quotedGroups[0] === '' &&
    quotedGroups.length % 2 === 1
  ) {
    return []
  }

  let brackets = 0
  for (const ch of nameToken) {
    if (ch === '[') {
      brackets++
    } else if (ch === ']') {
      brackets--
    }
  }

  if (brackets !== 0) {
    return [
      createCriticalProblem(
        'spec:name:unpaired-brackets',
        'unpaired brackets',
        line
      )
    ]
  }

  let name = nameToken
  /** @type {string | undefined} */
  let defaultValue

  if (name[0] === '[' && name.at(-1) === ']') {
    name = name.slice(1, -1)
    const parts = name.split('=')
    name = parts[0].trim()
    if (parts[1] !== undefined) {
      defaultValue = parts.slice(1).join('=').trim()
    }

    if (name === '') {
      return [
        createCriticalProblem('spec:name:empty-name', 'empty name', line)
      ]
    }
    if (defaultValue === '') {
      return [
        createCriticalProblem(
          'spec:name:empty-default',
          'empty default value',
          line
        )
      ]
    }
    if (
      defaultValue !== undefined &&
      !isQuoted(defaultValue) &&
      invalidDefault.test(defaultValue)
    ) {
      return [
        createCriticalProblem(
          'spec:name:invalid-default',
          'invalid default value syntax',
          line
        )
      ]
    }

    return []
  }

  const eqIndex = name.search(invalidDefault)
  if (eqIndex !== -1) {
    defaultValue = name.slice(eqIndex + 1).trim()
    name = name.slice(0, eqIndex).trim()

    if (name === '') {
      return [
        createCriticalProblem('spec:name:empty-name', 'empty name', line)
      ]
    }
    if (defaultValue === '') {
      return [
        createCriticalProblem(
          'spec:name:empty-default',
          'empty default value',
          line
        )
      ]
    }
    if (!isQuoted(defaultValue) && invalidDefault.test(defaultValue)) {
      return [
        createCriticalProblem(
          'spec:name:invalid-default',
          'invalid default value syntax',
          line
        )
      ]
    }
  }

  return []
}

/**
 * @param {import('comment-parser').Line[]} source
 * @returns {import('comment-parser').Problem[]}
 */
const getTagProblems = source => {
  if (source.length > 0 && source[0].tokens.tag === '') {
    return [
      createCriticalProblem(
        'spec:tag:prefix',
        'tag should start with "@" symbol',
        source[0].number
      )
    ]
  }

  return []
}

/**
 * @param {import('comment-parser').Line[]} source
 * @returns {import('comment-parser').Problem[]}
 */
const getTypeProblems = source => {
  let curlies = 0
  let sawType = false
  for (const line of source) {
    for (const ch of line.tokens.type) {
      sawType = true
      if (ch === '{') {
        curlies++
      } else if (ch === '}') {
        curlies--
      }
    }
  }

  if (sawType && curlies !== 0) {
    return [
      createCriticalProblem(
        'spec:type:unpaired-curlies',
        'unpaired curlies',
        source[0]?.number ?? 0
      )
    ]
  }

  return []
}

/**
 * @param {Array<{ message: string }>} diagnostics
 * @param {import('comment-parser').Line[]} source
 * @returns {import('comment-parser').Problem[]}
 */
const normalizeDiagnostics = (diagnostics, source) => {
  return diagnostics
    .filter(({ message }) => {
      // Covered by `spec:type:unpaired-curlies` on the owning tag.
      return message !== 'type expression is not closed'
    })
    .map(({ message }) => {
      return createCriticalProblem('custom', message, source[0]?.number ?? 0)
    })
}

/**
 * @param {Array<{ message: string, rootIndex: number }>} diagnostics
 * @returns {Map<number, Array<{ message: string, rootIndex: number }>>}
 */
const groupDiagnosticsByRootIndex = diagnostics => {
  const grouped = new Map()
  for (const diagnostic of diagnostics) {
    const group = grouped.get(diagnostic.rootIndex)
    if (group) {
      group.push(diagnostic)
    } else {
      grouped.set(diagnostic.rootIndex, [diagnostic])
    }
  }

  return grouped
}

/**
 * @param {string | {value: string, range?: [number, number]}} commentOrNode
 * @returns {number}
 */
const getCommentBaseOffset = commentOrNode => {
  if (
    typeof commentOrNode === 'object' &&
    commentOrNode !== null &&
    Array.isArray(commentOrNode.range)
  ) {
    return commentOrNode.range[0] ?? 0
  }

  return 0
}

/**
 * @param {Array<{index: number, problem: import('comment-parser').Problem}>} problems
 * @param {Array<import('.').JsdocBlockWithInline | null>} blocks
 * @returns {void}
 */
const throwBatchParseError = (problems, blocks) => {
  const firstProblem = problems[0]
  if (firstProblem) {
    throw new Error(
      `There were errors for comment batch parsing ` +
        `(index ${firstProblem.index}): ${firstProblem.problem.message}`
    )
  }

  const nullIndex = blocks.findIndex(block => {
    return block === null
  })
  if (nullIndex !== -1) {
    throw new Error(
      `There were no results for comment batch parsing (index ${nullIndex})`
    )
  }
}

/**
 * @param {object} tag
 * @returns {import('comment-parser').Spec}
 */
const normalizeTag = tag => {
  const name = tag.name ?? ''
  const source = Array.from(tag.source ?? [], normalizeLine)
  const nameToken = source[0]?.tokens.name ?? name
  const problems = [
    ...getTagProblems(source),
    ...getTypeProblems(source),
    ...getNameProblems(nameToken, source[0]?.number ?? 0)
  ]

  return {
    tag: tag.tag ?? '',
    name,
    type: tag.rawType ?? '',
    optional: problems.length === 0 && optionalBrackets.test(name),
    description: tag.description ?? '',
    inlineTags: [],
    problems,
    source
  }
}

/**
 * @param {object} block
 * @param {Array<{ message: string }>} diagnostics
 * @returns {import('.').JsdocBlockWithInline}
 */
const normalizeBlock = (block, diagnostics) => {
  const tags = Array.from(block.tags ?? [], normalizeTag)
  const source = Array.from(block.source ?? [], normalizeLine)
  const problems = [
    ...tags.flatMap(tag => tag.problems),
    ...normalizeDiagnostics(diagnostics, source)
  ]
  return parseInlineTags({
    description: block.description ?? '',
    tags,
    inlineTags: [],
    source,
    problems
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
  const { ast, diagnostics } = oxJsdocBinaryParse(sourceText, {
    compatMode: true,
    emptyStringForNull: true,
    preserveWhitespace: true
  })

  if (!ast) {
    throw new Error('There were no results for comment parsing')
  }

  return normalizeBlock(ast.toJSON(), diagnostics)
}

/**
 * Parse multiple comments with one `ox-jsdoc-binary` batch call and return
 * the same normalized block shape as `parseComment`.
 *
 * @param {Array<string | {value: string, range?: [number, number]}>} comments
 * @param {{indent?: string, throwOnError?: boolean}} [options]
 * @returns {{
 *   blocks: Array<import('.').JsdocBlockWithInline | null>,
 *   problems: Array<{index: number, problem: import('comment-parser').Problem}>
 * }}
 */
const parseCommentBatch = (comments, options = {}) => {
  const indent = options.indent ?? ''
  const items = comments.map(commentOrNode => {
    return {
      sourceText: normalizeCommentSource(commentOrNode, indent),
      baseOffset: getCommentBaseOffset(commentOrNode)
    }
  })
  const { asts, diagnostics } = oxJsdocBinaryParseBatch(items, {
    compatMode: true,
    emptyStringForNull: true,
    preserveWhitespace: true
  })
  const diagnosticsByIndex = groupDiagnosticsByRootIndex(diagnostics)
  /** @type {Array<import('.').JsdocBlockWithInline | null>} */
  const blocks = []
  /** @type {Array<{index: number, problem: import('comment-parser').Problem}>} */
  const problems = []

  for (const [index, ast] of asts.entries()) {
    const itemDiagnostics = diagnosticsByIndex.get(index) ?? []
    if (!ast) {
      const itemProblems = normalizeDiagnostics(itemDiagnostics, [])
      blocks.push(null)
      for (const problem of itemProblems) {
        problems.push({ index, problem })
      }
      continue
    }

    const block = normalizeBlock(ast.toJSON(), itemDiagnostics)
    blocks.push(block)
    for (const problem of block.problems) {
      problems.push({ index, problem })
    }
  }

  if (options.throwOnError && (problems.length > 0 || blocks.includes(null))) {
    throwBatchParseError(problems, blocks)
  }

  return {
    blocks,
    problems
  }
}

export { getTokenizers, parseComment, parseCommentBatch }
