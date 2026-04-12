/**
 * Standalone vitest configuration for napi binding tests.
 *
 * These tests CANNOT run through the root vite.config.ts `projects` setting
 * because vitest's VitestModuleRunner is unable to load native `.node`
 * binaries (napi addons). When vitest processes test files via its module
 * runner, `require()` calls for `.node` files inside `bindings.js` fail
 * with "Cannot read properties of undefined (reading 'config')".
 *
 * By using a dedicated config without `projects`, vitest falls back to its
 * default execution mode which can handle native module imports correctly.
 *
 * The root vite.config.ts `projects` array only contains the wasm browser
 * tests. This config is invoked separately via `pnpm --filter ox-jsdoc test`.
 */

import { defineConfig } from 'vite-plus'

export default defineConfig({
  test: {
    include: ['test/**/*.test.ts']
  }
})
