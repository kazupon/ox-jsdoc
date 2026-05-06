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
  return spec.tag === 'see' &&
    spec.source[0] !== undefined &&
    /\{@link.+?\}/v.test(spec.source[0].source)
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
const defaultNoTypesSet = new Set(defaultNoTypes)

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
const defaultNoNamesSet = new Set(defaultNoNames)

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
    start: tokens.start ?? '',
    delimiter: tokens.delimiter ?? '',
    postDelimiter: tokens.postDelimiter ?? '',
    tag: tokens.tag ?? '',
    postTag: tokens.postTag ?? '',
    name: tokens.name ?? '',
    postName: tokens.postName ?? '',
    type: tokens.type ?? '',
    postType: tokens.postType ?? '',
    description: tokens.description ?? '',
    end: tokens.end ?? '',
    lineEnd: tokens.lineEnd ?? ''
  }
}

/**
 * @param {object} line
 * @param {string[]} [sourceLines]
 * @returns {import('comment-parser').Line}
 */
const normalizeLine = (line, sourceLines) => {
  const tokens = normalizeTokens(line.tokens ?? {})
  const number = line.number ?? 0
  const source = sourceLines?.[number] ?? line.source ?? joinTokens(tokens)
  if (source.endsWith('\r')) {
    tokens.lineEnd = '\r'
  }

  return repairNestedDelimiterLine({
    number,
    source,
    tokens
  })
}

/**
 * @param {import('comment-parser').Tokens} tokens
 * @returns {string}
 */
const joinTokens = tokens => {
  return tokens.start +
    tokens.delimiter +
    tokens.postDelimiter +
    tokens.tag +
    tokens.postTag +
    tokens.type +
    tokens.postType +
    tokens.name +
    tokens.postName +
    tokens.description +
    tokens.end +
    tokens.lineEnd
}

/**
 * @param {string} value
 * @returns {boolean}
 */
const isQuoted = value => {
  return value !== '' && value.startsWith('"') && value.endsWith('"')
}

/**
 * @param {string} sourceText
 * @returns {string[]}
 */
const getSourceLines = sourceText => {
  return sourceText.split('\n')
}

/**
 * @param {import('comment-parser').Line} line
 * @returns {string}
 */
const getSourceWithoutLineEnd = line => {
  const { lineEnd } = line.tokens
  if (lineEnd !== '' && line.source.endsWith(lineEnd)) {
    return line.source.slice(0, -lineEnd.length)
  }

  return line.source
}

/**
 * @param {import('comment-parser').Line} line
 * @returns {string}
 */
const getSourceWithoutEnd = line => {
  const source = getSourceWithoutLineEnd(line)
  const { end } = line.tokens
  if (end !== '' && source.endsWith(end)) {
    return source.slice(0, -end.length)
  }

  return source
}

/**
 * @param {string} source
 * @param {string} lineEnd
 * @returns {string}
 */
const stripLineEnd = (source, lineEnd) => {
  if (lineEnd !== '' && source.endsWith(lineEnd)) {
    return source.slice(0, -lineEnd.length)
  }

  return source
}

/**
 * @param {string} remainder
 * @returns {{type: string, rest: string}}
 */
const parseLeadingType = remainder => {
  if (!remainder.startsWith('{')) {
    return {
      type: '',
      rest: remainder
    }
  }

  let curlies = 0
  for (const [index, ch] of Array.from(remainder).entries()) {
    if (ch === '{') {
      curlies++
    } else if (ch === '}') {
      curlies--
    }

    if (curlies === 0) {
      return {
        type: remainder.slice(0, index + 1),
        rest: remainder.slice(index + 1)
      }
    }
  }

  return {
    type: remainder,
    rest: ''
  }
}

/**
 * @param {string} source
 * @param {number} number
 * @param {string} lineEnd
 * @returns {import('comment-parser').Line}
 */
const tokenizeSourceLine = (source, number, lineEnd) => {
  const sourceWithoutLineEnd = stripLineEnd(source, lineEnd)
  const terminalMatch = sourceWithoutLineEnd.match(/^(\s*)\*\/$/v)
  if (terminalMatch) {
    return {
      number,
      source,
      tokens: normalizeTokens({
        start: terminalMatch[1],
        end: '*/',
        lineEnd
      })
    }
  }

  if (sourceWithoutLineEnd.startsWith('/**')) {
    return {
      number,
      source,
      tokens: normalizeTokens({
        delimiter: '/**',
        description: sourceWithoutLineEnd.slice(3),
        lineEnd
      })
    }
  }

  const bodyMatch = sourceWithoutLineEnd.match(/^(\s*)(\*)(\s*)(.*)$/v)
  const start = bodyMatch?.[1] ?? ''
  const delimiter = bodyMatch?.[2] ?? ''
  const postDelimiter = bodyMatch?.[3] ?? ''
  const body = bodyMatch?.[4] ?? sourceWithoutLineEnd
  const tagMatch = body.match(/^(@\S+)(\s*)(.*)$/v)

  if (!tagMatch || tagMatch[1].includes('/')) {
    return {
      number,
      source,
      tokens: normalizeTokens({
        start,
        delimiter,
        postDelimiter,
        description: body,
        lineEnd
      })
    }
  }

  const tag = tagMatch[1]
  const postTag = tagMatch[2]
  const { type, rest } = parseLeadingType(tagMatch[3])
  const [postType, nameRemainder] = splitLeadingWhitespace(rest)
  const nameParts = splitNameRemainder(nameRemainder)

  return {
    number,
    source,
    tokens: normalizeTokens({
      start,
      delimiter,
      postDelimiter,
      tag,
      postTag,
      type,
      postType,
      name: nameParts.name,
      postName: nameParts.postName,
      description: nameParts.description,
      lineEnd
    })
  }
}

/**
 * @param {import('comment-parser').Line} line
 * @returns {import('comment-parser').Line}
 */
const repairNestedDelimiterLine = line => {
  if (line.tokens.delimiter !== '/**') {
    return line
  }

  const openerIndex = line.source.indexOf('/**')
  const tagIndex = line.source.indexOf('@')
  if (tagIndex === -1 || openerIndex === -1 || tagIndex > openerIndex) {
    return line
  }

  return tokenizeSourceLine(line.source, line.number, line.tokens.lineEnd)
}

/**
 * @param {import('comment-parser').Line} line
 * @returns {string}
 */
const getRemainderAfterTag = line => {
  const { tokens } = line
  const prefix = tokens.start +
    tokens.delimiter +
    tokens.postDelimiter +
    tokens.tag +
    tokens.postTag

  return getSourceWithoutEnd(line).slice(prefix.length)
}

/**
 * @param {import('comment-parser').Line} line
 * @returns {string}
 */
const getRemainderAfterDelimiter = line => {
  const { tokens } = line
  const prefix = tokens.start +
    tokens.delimiter +
    tokens.postDelimiter

  return getSourceWithoutEnd(line).slice(prefix.length)
}

/**
 * @param {import('comment-parser').Line} line
 * @returns {void}
 */
const convertLineToDescription = line => {
  line.tokens.description = getRemainderAfterDelimiter(line)
  line.tokens.tag = ''
  line.tokens.postTag = ''
  line.tokens.name = ''
  line.tokens.postName = ''
  line.tokens.type = ''
  line.tokens.postType = ''
}

/**
 * @param {import('comment-parser').Line} line
 * @returns {boolean}
 */
const hasFenceMarker = line => {
  return line.tokens.name.trim().startsWith('```') ||
    line.tokens.description.trim().startsWith('```')
}

/**
 * @param {import('comment-parser').Line[]} source
 * @returns {void}
 */
const repairSourceLines = source => {
  let inFence = false
  for (const line of source) {
    if (inFence) {
      convertLineToDescription(line)
      if (hasFenceMarker(line)) {
        inFence = false
      }
      continue
    }

    if (line.tokens.tag.includes('/')) {
      convertLineToDescription(line)
    }

    if (hasFenceMarker(line)) {
      inFence = true
    }
  }
}

/**
 * @param {import('comment-parser').Line} line
 * @returns {string}
 */
const getRemainderAfterType = line => {
  const { tokens } = line
  const prefix = tokens.start +
    tokens.delimiter +
    tokens.postDelimiter +
    tokens.tag +
    tokens.postTag +
    tokens.type +
    tokens.postType

  return getSourceWithoutEnd(line).slice(prefix.length)
}

/**
 * @param {string} value
 * @returns {[string, string]}
 */
const splitLeadingWhitespace = value => {
  const match = value.match(/^(\s*)(.*)$/v)
  return [
    match?.[1] ?? '',
    match?.[2] ?? value
  ]
}

/**
 * @param {string} remainder
 * @returns {{name: string, postName: string, description: string}}
 */
const splitNameRemainder = remainder => {
  if (remainder === '') {
    return {
      name: '',
      postName: '',
      description: ''
    }
  }

  let nameEnd = -1
  if (remainder.startsWith('[')) {
    const closingBracket = remainder.indexOf(']')
    if (closingBracket !== -1) {
      nameEnd = closingBracket + 1
    }
  }

  if (nameEnd === -1) {
    const whitespace = remainder.search(/\s/v)
    nameEnd = whitespace === -1 ? remainder.length : whitespace
  }

  const name = remainder.slice(0, nameEnd)
  const [postName, description] = splitLeadingWhitespace(
    remainder.slice(nameEnd)
  )

  return {
    name,
    postName,
    description
  }
}

/**
 * @param {string} nameToken
 * @returns {{name: string, optional: boolean, defaultValue?: string}}
 */
const parseNameToken = nameToken => {
  if (nameToken.startsWith('[') && nameToken.endsWith(']')) {
    const innerName = nameToken.slice(1, -1)
    const parts = innerName.split('=')
    const name = parts[0].trim()
    if (parts[1] === undefined) {
      return {
        name,
        optional: true
      }
    }

    return {
      name,
      optional: true,
      defaultValue: parts.slice(1).join('=').trim()
    }
  }

  const eqIndex = nameToken.search(invalidDefault)
  if (eqIndex === -1) {
    return {
      name: nameToken,
      optional: false
    }
  }

  return {
    name: nameToken.slice(0, eqIndex).trim(),
    optional: false,
    defaultValue: nameToken.slice(eqIndex + 1).trim()
  }
}

/**
 * @param {import('comment-parser').Line[]} source
 * @returns {string}
 */
const getJoinedDescription = source => {
  let start = 0
  while (source[start]?.tokens.description === '') {
    start++
  }

  let end = source.length - 1
  while (end >= start && source[end]?.tokens.description === '') {
    end--
  }

  let description = ''
  for (let index = start; index <= end; index++) {
    if (index > start) {
      description += '\n'
    }
    description += source[index].tokens.description
  }

  return description
}

/**
 * @param {import('comment-parser').Line[]} source
 * @param {number} [endIndex]
 * @returns {string}
 */
const getJoinedBlockDescription = (source, endIndex = source.length) => {
  let description = ''
  for (let index = 0; index < endIndex; index++) {
    const value = source[index].tokens.description
    if (value === '') {
      continue
    }
    description += description === '' ? value : ` ${value}`
  }

  return description
}

/**
 * @param {string} rawType
 * @returns {string}
 */
const stripEncapsulatingCurlies = rawType => {
  if (rawType.startsWith('{') && rawType.endsWith('}')) {
    return rawType.slice(1, -1)
  }

  return rawType
}

/**
 * @param {import('comment-parser').Line[]} source
 * @returns {string}
 */
const getTagType = source => {
  let rawType = ''
  for (const line of source) {
    if (line.tokens.type === '') {
      continue
    }
    rawType += rawType === '' ? line.tokens.type : `\n${line.tokens.type}`
  }

  return stripEncapsulatingCurlies(rawType)
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
  const tagName = tag.tag ?? ''
  const source = tag.source ?? []
  const seeWithLink = hasSeeWithLink({
    tag: tagName,
    source
  })

  for (const line of source) {
    if (defaultNoTypesSet.has(tagName) && line.tokens.type !== '') {
      line.tokens.description = line.tokens.type +
        line.tokens.postType +
        line.tokens.description
      line.tokens.type = ''
      line.tokens.postType = ''
    }
  }

  if (
    defaultNoNamesSet.has(tagName) ||
    seeWithLink
  ) {
    for (const line of source) {
      if (line.tokens.name !== '') {
        line.tokens.description = line.tokens.name +
          line.tokens.postName +
          line.tokens.description
        line.tokens.name = ''
        line.tokens.postName = ''
      }
    }
  }

  const tagProblems = getTagProblems(source)
  const typeProblems = getTypeProblems(source)
  if (typeProblems.length > 0) {
    const typeLine = source.find(line => {
      return line.tokens.tag !== ''
    }) ?? source[0]
    if (typeLine) {
      typeLine.tokens.description = getRemainderAfterTag(typeLine)
      typeLine.tokens.type = ''
      typeLine.tokens.postType = ''
      typeLine.tokens.name = ''
      typeLine.tokens.postName = ''
    }
    const problems = [
      ...tagProblems,
      ...typeProblems
    ]

    return {
      tag: tagName,
      name: '',
      type: '',
      optional: false,
      description: '',
      inlineTags: [],
      problems,
      source
    }
  }

  const type = getTagType(source)
  /** @type {{name: string, optional: boolean, defaultValue?: string, problems: import('comment-parser').Problem[], critical: boolean, descriptionPrefix: string}} */
  const nameInfo = {
    name: tag.name ?? '',
    optional: tagName === 'template' && optionalBrackets.test(tag.name ?? ''),
    problems: [],
    critical: false,
    descriptionPrefix: ''
  }

  if (
    tagName !== 'template' &&
    !defaultNoNamesSet.has(tagName) &&
    !seeWithLink
  ) {
    let nameLine = source.find(line => {
      return line.tokens.tag !== '' && line.tokens.name !== ''
    }) ?? source.find(line => {
      return line.tokens.name !== ''
    })

    if (!nameLine) {
      const continuationNameLine = source.find((line, index) => {
        return index > 0 &&
          line.tokens.tag === '' &&
          line.tokens.name === '' &&
          line.tokens.type === '' &&
          line.tokens.description.trim() !== '' &&
          line.tokens.end === ''
      })
      if (continuationNameLine) {
        const nameParts = splitNameRemainder(
          continuationNameLine.tokens.description
        )
        if (nameParts.name !== '') {
          continuationNameLine.tokens.name = nameParts.name
          continuationNameLine.tokens.postName = nameParts.postName
          continuationNameLine.tokens.description = nameParts.description
          nameInfo.descriptionPrefix =
            '\n' + continuationNameLine.tokens.postDelimiter.slice(1)
          nameLine = continuationNameLine
        }
      }
    }

    if (nameLine) {
      const nameParts = nameLine.tokens.tag === ''
        ? {
          name: nameLine.tokens.name,
          postName: nameLine.tokens.postName,
          description: nameLine.tokens.description
        }
        : splitNameRemainder(getRemainderAfterType(nameLine))

      nameLine.tokens.name = nameParts.name
      nameLine.tokens.postName = nameParts.postName
      nameLine.tokens.description = nameParts.description

      const nameProblems = getNameProblems(
        nameParts.name,
        nameLine.number
      )

      if (nameProblems.length > 0) {
        nameLine.tokens.name = ''
        nameLine.tokens.postName = ''
        nameLine.tokens.description =
          nameParts.name + nameParts.postName + nameParts.description
        nameInfo.name = ''
        nameInfo.optional = false
        nameInfo.problems = nameProblems
        nameInfo.critical = true
      } else {
        const parsedName = parseNameToken(nameParts.name)
        nameInfo.name = parsedName.name
        nameInfo.optional = parsedName.optional
        if (parsedName.defaultValue !== undefined) {
          nameInfo.defaultValue = parsedName.defaultValue
        }
      }
    }
  }

  const problems = [
    ...tagProblems,
    ...nameInfo.problems
  ]

  /** @type {import('comment-parser').Spec} */
  const spec = {
    tag: tagName,
    name: nameInfo.name,
    type,
    optional: problems.length === 0 && nameInfo.optional,
    description: nameInfo.critical
      ? ''
      : nameInfo.descriptionPrefix + getJoinedDescription(source),
    inlineTags: [],
    problems,
    source
  }

  if (nameInfo.defaultValue !== undefined && problems.length === 0) {
    spec.default = nameInfo.defaultValue
  }

  return spec
}

/**
 * @param {string} tagToken
 * @returns {string}
 */
const getTagNameFromToken = tagToken => {
  return tagToken.startsWith('@') ? tagToken.slice(1) : tagToken
}

/**
 * @param {object} block
 * @param {import('comment-parser').Line[]} source
 * @returns {object[]}
 */
const buildTagInputs = (block, source) => {
  const rawTagByLine = new Map()
  for (const tag of block.tags ?? []) {
    const firstLineNumber = tag.source?.[0]?.number
    if (firstLineNumber !== undefined) {
      rawTagByLine.set(firstLineNumber, tag)
    }
  }

  const tagStarts = []
  for (const [index, line] of source.entries()) {
    if (line.tokens.tag !== '') {
      tagStarts.push({
        line,
        index
      })
    }
  }

  const tagInputs = []
  for (const [tagIndex, {
    line,
    index
  }] of tagStarts.entries()) {
    const nextIndex = tagStarts[tagIndex + 1]?.index ?? source.length
    const rawTag = rawTagByLine.get(line.number) ?? {}

    tagInputs.push({
      ...rawTag,
      tag: rawTag.tag ?? getTagNameFromToken(line.tokens.tag),
      name: rawTag.name ?? line.tokens.name,
      source: source.slice(index, nextIndex)
    })
  }

  return tagInputs
}

/**
 * @param {object} block
 * @param {Array<{ message: string }>} diagnostics
 * @param {string} sourceText
 * @returns {import('.').JsdocBlockWithInline}
 */
const normalizeBlock = (block, diagnostics, sourceText) => {
  void diagnostics
  const sourceLines = getSourceLines(sourceText)
  const source = Array.from(block.source ?? [], line => {
    return normalizeLine(line, sourceLines)
  })
  repairSourceLines(source)
  const tagInputs = buildTagInputs(block, source)
  const tags = Array.from(tagInputs, tag => {
    return normalizeTag(tag)
  })
  const firstTagSource = tags[0]?.source[0]
  const rawDescriptionEndIndex = firstTagSource === undefined
    ? source.length
    : source.indexOf(firstTagSource)
  const descriptionEndIndex = rawDescriptionEndIndex === -1
    ? source.length
    : rawDescriptionEndIndex
  const problems = []
  for (const tag of tags) {
    problems.push(...tag.problems)
  }
  return parseInlineTags({
    description: getJoinedBlockDescription(source, descriptionEndIndex),
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

  return normalizeBlock(ast.toJSON(), diagnostics, sourceText)
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

    const block = normalizeBlock(ast.toJSON(), itemDiagnostics, items[index].sourceText)
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
