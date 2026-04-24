/**
 * Fixture comments for the jsdoccomment-compat dynamic comparison test.
 *
 * Each fixture is a complete `/** ... *​/` JSDoc block string. The Level 2
 * test feeds the same string into both `@es-joy/jsdoccomment` and
 * `ox-jsdoc-binary` (compat_mode + emptyStringForNull) and compares the
 * resulting ESTree-shape AST field-by-field.
 *
 * @author kazuya kawaguchi (a.k.a. kazupon)
 * @license MIT
 */

export interface Fixture {
  name: string
  source: string
}

export const FIXTURES: ReadonlyArray<Fixture> = [
  {
    name: 'description-only single line',
    source: '/** Hello world */'
  },
  {
    name: 'description-only multi-line',
    source: '/**\n * Hello\n * world\n */'
  },
  {
    name: 'param tag with type and name',
    source: '/**\n * @param {string} id - User ID\n */'
  },
  {
    name: 'returns tag with type only',
    source: '/**\n * @returns {boolean} True if valid\n */'
  },
  {
    name: 'multiple tags',
    source:
      '/**\n * Description\n * @param {string} id\n * @param {number} count\n * @returns {boolean}\n */'
  },
  {
    name: 'inline link',
    source: '/**\n * See {@link Foo} for details\n */'
  },
  {
    name: 'no description, single tag',
    source: '/** @returns void */'
  },
  {
    name: 'tag with multi-line description',
    source: '/**\n * @param {string} id - User ID\n *   continued on next line\n */'
  },
  {
    name: 'returns void (defaultNoNames)',
    source: '/** @returns void */'
  },
  {
    name: 'example tag (defaultNoTypes + defaultNoNames)',
    source: '/** @example const x = 1 */'
  },
  {
    name: 'see with link (hasSeeWithLink)',
    source: '/** @see {@link Foo} for details */'
  }
]
