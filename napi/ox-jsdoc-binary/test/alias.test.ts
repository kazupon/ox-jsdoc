/**
 * Smoke test for the JS-only `ox-jsdoc-binary` alias package. Verifies that
 * the alias re-exports the canonical `ox-jsdoc` surface and that a basic
 * `parse()` call goes through to the canonical NAPI binding unchanged.
 *
 * @author kazuya kawaguchi (a.k.a. kazupon)
 * @license MIT
 */

import type { RemoteJsdocBlock, RemoteJsdocTag, RemoteJsdocTagName } from '@ox-jsdoc/decoder'
import { describe, expect, it } from 'vite-plus/test'
import * as alias from '../src-js/index.js'

describe('ox-jsdoc-binary alias', () => {
  it('re-exports the canonical NAPI surface', () => {
    const expected = ['parse', 'parseBatch', 'parseType', 'parseTypeCheck', 'jsdocVisitorKeys']
    const actual = Object.keys(alias).sort()
    for (const name of expected) {
      expect(actual.includes(name), `missing export: ${name}`).toBe(true)
    }
  })

  it('parse() goes through to the canonical implementation', () => {
    const result = alias.parse('/** @param {string} id - The user ID */')
    expect(result.diagnostics).toEqual([])
    expect(result.ast).not.toBeNull()
    const ast = result.ast as RemoteJsdocBlock
    const tag = ast.tags[0] as RemoteJsdocTag
    expect((tag.tag as RemoteJsdocTagName).value).toBe('param')
    expect(tag.description).toBe('The user ID')
  })
})
