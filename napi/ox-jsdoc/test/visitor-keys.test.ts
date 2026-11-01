import { describe, expect, it } from 'vite-plus/test'

import { jsdocVisitorKeys, parse, parseBatch } from '../src-js/index.js'

const visitorKeys = jsdocVisitorKeys as Readonly<Record<string, readonly string[]>>

interface Node {
  readonly type: string
  readonly parent: unknown
  readonly [key: string]: unknown
  toJSON(): unknown
}

const isNode = (value: unknown): value is Node =>
  value !== null && typeof value === 'object' && typeof (value as Node).type === 'string'

/** How many nodes of each kind a walk along the visitor keys reaches from the roots. */
function walk(roots: ReadonlyArray<unknown>): Record<string, number> {
  const counts: Record<string, number> = {}
  const stack = roots.filter(isNode)
  while (stack.length > 0) {
    const node = stack.pop()!
    counts[node.type] = (counts[node.type] ?? 0) + 1
    for (const key of visitorKeys[node.type] ?? []) {
      const child = node[key]
      if (Array.isArray(child)) {
        stack.push(...child.filter(isNode))
      } else if (isNode(child)) {
        stack.push(child)
      }
    }
  }
  return counts
}

/** How many nodes of each kind the buffer holds, from its node records (no NodeList wrappers). */
function inventory(sourceFile: {
  readonly nodeCount: number
  getNode(index: number, parent: null, rootIndex: number): unknown
}): Record<string, number> {
  const counts: Record<string, number> = {}
  for (let index = 1; index < sourceFile.nodeCount; index++) {
    const { type } = sourceFile.getNode(index, null, 0) as Node
    if (type !== 'NodeList') {
      counts[type] = (counts[type] ?? 0) + 1
    }
  }
  return counts
}

/** Tags of each kind of value (parameter names, namepaths, text), inline tags and types (with method signatures). */
const sources = [
  '/** A description with {@link Foo} and {@link Bar the bar}. */',
  '/**\n * @param {string} id - The id.\n * @param {number} [count=1] How many.\n * @returns {Promise<void>} Nothing.\n */',
  '/** @param name */',
  '/** @param 名前 説明 */',
  '/** @param foo-bar */',
  '/** @see module:foo/bar */',
  '/** @fires Foo#event */',
  '/** @typedef {{ a: string, b?: number }} Foo */',
  '/** @type {(a: string) => void} */',
  '/** @type {Array<string | number> | null} */',
  '/** @type {{ foo<T, U>(a: T, b: number): U, bar(): void, baz<T>(): T }} */',
  '/** @template T */',
  '/** @deprecated */',
  'not a comment'
]

const modes = {
  basic: {},
  compat: { compatMode: true, emptyStringForNull: true, preserveWhitespace: true },
  'parsed types': { parseTypes: true, typeParseMode: 'typescript' as const }
}

describe('a walk along the visitor keys', () => {
  for (const [mode, options] of Object.entries(modes)) {
    it(`reaches every node of the buffer (${mode})`, () => {
      const result = parseBatch(
        sources.map(sourceText => ({ sourceText })),
        options
      )
      expect(walk(result.asts)).toEqual(inventory(result.sourceFile))
    })
  }
})

describe('JsdocGenericTagBody', () => {
  /** The body of the first tag. */
  const bodyOf = (sourceText: string) =>
    (parse(sourceText).ast!.tags[0] as unknown as { readonly body: unknown }).body as Node & {
      readonly typeSource: Node | null
      readonly value: Node | null
    }

  it('reads the type source and a parameter name', () => {
    const body = bodyOf('/** @param {number} [count=1] How many. */')
    expect(body.typeSource).toMatchObject({ type: 'JsdocTypeSource', raw: 'number' })
    expect(body.value).toMatchObject({
      type: 'JsdocParameterName',
      path: 'count',
      optional: true,
      defaultValue: '1'
    })
    expect(body.value!.parent).toBe(body)
  })

  it('reads a namepath and a text value', () => {
    expect(bodyOf('/** @see module:foo/bar */').value).toMatchObject({
      type: 'JsdocNamepathSource',
      raw: 'module:foo/bar'
    })
    expect(bodyOf('/** @param 名前 説明 */').value).toMatchObject({
      type: 'JsdocText',
      value: '名前'
    })
    expect(bodyOf('/** @deprecated */')).toBeNull()
  })

  it('leaves its children out of toJSON(), which the tag has as rawType and name', () => {
    expect(Object.keys(bodyOf('/** @param {string} id */').toJSON() as object)).toEqual([
      'type',
      'range',
      'hasDashSeparator',
      'description'
    ])
  })
})
