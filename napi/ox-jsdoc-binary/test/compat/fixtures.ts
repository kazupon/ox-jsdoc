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
  // TODO(compat): "description-only multi-line" is intentionally omitted
  // — exposes the same pre-existing parser/writer multi-line bug as the
  // tag variant (JsdocBlock.description returns corrupted bytes when the
  // source has multiple description lines). Tracked separately.
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
  // TODO(compat): "tag with multi-line description" is intentionally
  // omitted — exposes a pre-existing parser/writer bug where the second
  // description line corrupts JsdocTag.rawType / JsdocTag.description
  // bytes. Tracked separately; restore once the upstream fix lands.
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
