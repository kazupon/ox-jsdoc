/**
 * End-to-end coverage for `descriptionRaw` / `descriptionText(preserveWhitespace)`
 * on the JS decoder, exercising the full Rust parser → Binary writer →
 * JS lazy decoder pipeline.
 *
 * Phase 5: `descriptionRaw` is gated on the `preserveWhitespace` parse
 * option (per-node Common Data bit), fully orthogonal to `compatMode`.
 * See `design/008-oxlint-oxfmt-support/README.md` §4.2 / §4.3.
 *
 * The Rust-side coverage lives in `crates/ox_jsdoc/tests/description_raw.rs`;
 * this file mirrors the essential cases on the JS side so that any
 * wire-format / writer drift surfaces in CI.
 *
 * @author kazuya kawaguchi (a.k.a. kazupon)
 * @license MIT
 */

import type { RemoteJsdocBlock, RemoteJsdocTag } from '@ox-jsdoc/decoder'
import { describe, expect, it } from 'vite-plus/test'

import { parse } from '../src-js/index.js'

// ---------------------------------------------------------------------------
// Default (preserveWhitespace = false) — descriptionRaw / descriptionText(true)
// must be null regardless of compatMode (the per-node bit is clear).
// ---------------------------------------------------------------------------

describe('without preserveWhitespace opt-in', () => {
  it('descriptionRaw is null on basic-mode buffer with description', () => {
    const result = parse('/** Hello world */')
    const block = result.ast as RemoteJsdocBlock
    expect(block.descriptionRaw).toBeNull()
  })

  it('descriptionRaw is null on compat-mode buffer (without preserveWhitespace)', () => {
    const result = parse('/** Hello world */', { compatMode: true })
    const block = result.ast as RemoteJsdocBlock
    expect(block.descriptionRaw).toBeNull()
  })

  it('descriptionRaw is null for tag with description (basic)', () => {
    const result = parse('/** @param x A short desc */')
    const tag = (result.ast as RemoteJsdocBlock).tags[0] as RemoteJsdocTag
    expect(tag.descriptionRaw).toBeNull()
  })

  it('descriptionText(false) returns the compact description', () => {
    const result = parse('/** Hello world */')
    const block = result.ast as RemoteJsdocBlock
    expect(block.descriptionText(false)).toBe('Hello world')
    expect(block.descriptionText()).toBe('Hello world')
  })

  it('descriptionText(true) returns null without preserveWhitespace opt-in', () => {
    const result = parse('/**\n * Multi-line\n * description.\n */')
    const block = result.ast as RemoteJsdocBlock
    expect(block.descriptionText(true)).toBeNull()
  })
})

// ---------------------------------------------------------------------------
// preserveWhitespace: true (basic mode) — descriptionRaw is the byte-exact
// source slice. compatMode does not need to be on.
// ---------------------------------------------------------------------------

describe('basic-mode + preserveWhitespace JsdocBlock.descriptionRaw', () => {
  it('returns null when the block has no description', () => {
    const result = parse('/** @param x */', { preserveWhitespace: true })
    const block = result.ast as RemoteJsdocBlock
    expect(block.descriptionRaw).toBeNull()
    expect(block.descriptionText(true)).toBeNull()
  })

  it('returns the byte-exact slice for single-line description', () => {
    const result = parse('/** Just one line */', { preserveWhitespace: true })
    const block = result.ast as RemoteJsdocBlock
    // Single-line: trailing whitespace before `*/` is preserved verbatim.
    expect(block.descriptionRaw).toBe('Just one line ')
    // Algorithm trims it.
    expect(block.descriptionText(true)).toBe('Just one line')
  })

  it('preserves intermediate `* ` margins in multi-line slice', () => {
    const result = parse('/**\n * First line.\n * Second line.\n */', {
      preserveWhitespace: true
    })
    const block = result.ast as RemoteJsdocBlock
    expect(block.descriptionRaw).toBe('First line.\n * Second line.')
    expect(block.description).toBe('First line.\nSecond line.')
  })

  it('preserves blank-line margins (paragraph break) in raw slice', () => {
    const result = parse('/**\n * First.\n *\n * Second.\n */', {
      preserveWhitespace: true
    })
    const block = result.ast as RemoteJsdocBlock
    expect(block.descriptionRaw).toBe('First.\n *\n * Second.')
    expect(block.descriptionText(true)).toBe('First.\n\nSecond.')
  })

  it('preserves indented code blocks past the `* ` prefix', () => {
    const src = '/**\n * Intro.\n *\n *     code()\n *\n * Outro.\n */'
    const result = parse(src, { preserveWhitespace: true })
    const block = result.ast as RemoteJsdocBlock
    expect(block.descriptionText(true)).toBe('Intro.\n\n    code()\n\nOutro.')
  })
})

describe('basic-mode + preserveWhitespace JsdocTag.descriptionRaw', () => {
  it('returns null when the tag has no description body', () => {
    const result = parse('/** @param {T} id */', { preserveWhitespace: true })
    const tag = (result.ast as RemoteJsdocBlock).tags[0] as RemoteJsdocTag
    expect(tag.descriptionRaw).toBeNull()
    expect(tag.descriptionText(true)).toBeNull()
  })

  it('returns the byte-exact slice for single-line tag description', () => {
    const result = parse('/** @param {T} id A short description */', {
      preserveWhitespace: true
    })
    const tag = (result.ast as RemoteJsdocBlock).tags[0] as RemoteJsdocTag
    expect(tag.descriptionRaw).toBe('A short description ')
    expect(tag.descriptionText(true)).toBe('A short description')
  })

  it('uses the corrected end offset for multi-line tag descriptions', () => {
    const src = '/**\n * @param {T} x first line of desc\n *   continuation here\n */'
    const result = parse(src, { preserveWhitespace: true })
    const tag = (result.ast as RemoteJsdocBlock).tags[0] as RemoteJsdocTag
    const raw = tag.descriptionRaw
    expect(raw).not.toBeNull()
    expect(raw!.startsWith('first line of desc')).toBe(true)
    expect(raw!.endsWith('continuation here')).toBe(true)
    expect(raw!.includes('\n *   ')).toBe(true)
    expect(tag.descriptionText(true)).toBe('first line of desc\n  continuation here')
  })

  it('preserves paragraph breaks in tag description', () => {
    const src = '/**\n * @param x first paragraph.\n *\n * second paragraph.\n */'
    const result = parse(src, { preserveWhitespace: true })
    const tag = (result.ast as RemoteJsdocBlock).tags[0] as RemoteJsdocTag
    expect(tag.descriptionText(true)).toBe('first paragraph.\n\nsecond paragraph.')
  })
})

// ---------------------------------------------------------------------------
// compatMode + preserveWhitespace combination — descriptionRaw works AND
// the compat tail (delimiters / line indices) is also present.
// ---------------------------------------------------------------------------

describe('compatMode + preserveWhitespace combination', () => {
  it('descriptionRaw works on JsdocBlock', () => {
    const result = parse('/** Just one line */', {
      compatMode: true,
      preserveWhitespace: true
    })
    const block = result.ast as RemoteJsdocBlock
    expect(block.descriptionRaw).toBe('Just one line ')
    expect(block.descriptionText(true)).toBe('Just one line')
    // compat tail still populated.
    expect(block.endLine).toBe(0)
  })

  it('descriptionRaw works on JsdocTag', () => {
    const result = parse('/** @param {T} id A short description */', {
      compatMode: true,
      preserveWhitespace: true
    })
    const tag = (result.ast as RemoteJsdocBlock).tags[0] as RemoteJsdocTag
    expect(tag.descriptionRaw).toBe('A short description ')
    expect(tag.descriptionText(true)).toBe('A short description')
  })
})

// ---------------------------------------------------------------------------
// JSON serializer parity — descriptionRaw shows up in compat-mode toJSON
// only when present (matches the Rust serializer's
// `skip_serializing_if = "Option::is_none"`). Phase 5: presence further
// requires `preserveWhitespace: true` at parse time.
// ---------------------------------------------------------------------------

describe('toJSON descriptionRaw emission', () => {
  it('omits descriptionRaw on basic-mode buffer', () => {
    const result = parse('/**\n * Multi-line\n * description.\n */')
    const json = (result.ast as RemoteJsdocBlock).toJSON() as Record<string, unknown>
    expect('descriptionRaw' in json).toBe(false)
  })

  it('omits descriptionRaw on compat-mode buffer without preserveWhitespace', () => {
    const result = parse('/**\n * Multi-line\n * description.\n */', {
      compatMode: true
    })
    const json = (result.ast as RemoteJsdocBlock).toJSON() as Record<string, unknown>
    expect('descriptionRaw' in json).toBe(false)
  })

  it('emits descriptionRaw only when compatMode + preserveWhitespace are both on', () => {
    const result = parse('/**\n * Multi-line\n * description.\n */', {
      compatMode: true,
      preserveWhitespace: true
    })
    const json = (result.ast as RemoteJsdocBlock).toJSON() as Record<string, unknown>
    expect(json.descriptionRaw).toBe('Multi-line\n * description.')
  })

  it('omits descriptionRaw on compat-mode buffer when block has no description', () => {
    const result = parse('/** @param x */', {
      compatMode: true,
      preserveWhitespace: true
    })
    const json = (result.ast as RemoteJsdocBlock).toJSON() as Record<string, unknown>
    expect('descriptionRaw' in json).toBe(false)
  })

  it('emits descriptionRaw on JsdocTag when compatMode + preserveWhitespace are both on', () => {
    const src = '/**\n * @param {T} x first\n *   continuation\n */'
    const result = parse(src, { compatMode: true, preserveWhitespace: true })
    const blockJson = (result.ast as RemoteJsdocBlock).toJSON() as Record<string, unknown>
    const tags = blockJson.tags as Array<Record<string, unknown>>
    expect(tags).toHaveLength(1)
    const raw = tags[0].descriptionRaw as string
    expect(raw.startsWith('first')).toBe(true)
    expect(raw.endsWith('continuation')).toBe(true)
  })
})
