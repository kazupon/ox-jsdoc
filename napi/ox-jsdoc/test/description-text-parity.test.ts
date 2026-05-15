/**
 * Cross-language parity for `descriptionText(preserveWhitespace)` —
 * asserts the JS Binary AST decoder produces the **same** output as the
 * Rust typed-AST API for every fixture in
 * `fixtures/cross-language/description-text.json`.
 *
 * The matching Rust side lives in
 * `crates/ox_jsdoc/tests/cross_language_parity.rs` and asserts against
 * the **same JSON file** — so any algorithm divergence between the two
 * implementations surfaces as a CI failure on one (or both) sides.
 *
 * See `design/008-oxlint-oxfmt-support/README.md` §7.3.
 *
 * @author kazuya kawaguchi (a.k.a. kazupon)
 * @license MIT
 */

import { readFileSync } from 'node:fs'
import { dirname, resolve } from 'node:path'
import { fileURLToPath } from 'node:url'

import type { RemoteJsdocBlock, RemoteJsdocTag } from '@ox-jsdoc/decoder'
import { describe, expect, it } from 'vite-plus/test'

import { parse } from '../src-js/index.js'

interface Expected {
  compact: string | null
  preserve: string | null
}

interface Fixture {
  name: string
  source: string
  block: Expected
  tag: Expected | null
}

interface FixtureFile {
  fixtures: Fixture[]
}

const __dirname = dirname(fileURLToPath(import.meta.url))
const FIXTURE_PATH = resolve(__dirname, '../../../fixtures/cross-language/description-text.json')

const file = JSON.parse(readFileSync(FIXTURE_PATH, 'utf8')) as FixtureFile

describe('JS descriptionText matches shared cross-language fixtures', () => {
  for (const fx of file.fixtures) {
    it(`fixture: ${fx.name}`, () => {
      // preserveWhitespace is required so the descriptionRaw wire field
      // is present (the preserve path needs it). compatMode is orthogonal —
      // we test the basic-mode path here for the leanest opt-in.
      const result = parse(fx.source, { preserveWhitespace: true })
      const block = result.ast as RemoteJsdocBlock
      expect(block, `parse failed for source: ${JSON.stringify(fx.source)}`).not.toBeNull()

      expect(block.descriptionText(false), `block.descriptionText(false)`).toBe(fx.block.compact)
      expect(block.descriptionText(true), `block.descriptionText(true)`).toBe(fx.block.preserve)

      if (fx.tag !== null) {
        expect(block.tags.length, `expected at least one tag for ${fx.name}`).toBeGreaterThan(0)
        const tag = block.tags[0] as RemoteJsdocTag
        expect(tag.descriptionText(false), `tag.descriptionText(false)`).toBe(fx.tag.compact)
        expect(tag.descriptionText(true), `tag.descriptionText(true)`).toBe(fx.tag.preserve)
      }
    })
  }
})
