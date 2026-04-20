/**
 * Standalone vitest config for @ox-jsdoc/decoder.
 *
 * Runs in pure-JS mode — these tests do not load any native binding,
 * they construct DataView fixtures directly and exercise the lazy
 * decoder against them.
 */

import { defineConfig } from 'vite-plus'

export default defineConfig({
  test: {
    include: ['test/**/*.test.js']
  }
})
