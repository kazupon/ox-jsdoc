/**
 * Standalone vitest configuration for the ox-jsdoc-binary NAPI binding tests.
 *
 * Uses the same dedicated config approach as napi/ox-jsdoc — vitest's
 * VitestModuleRunner cannot load native `.node` binaries through the root
 * `projects` setting, so this binding ships its own vitest config.
 */

import { defineConfig } from 'vite-plus'

export default defineConfig({
  test: {
    include: ['test/**/*.test.ts']
  }
})
