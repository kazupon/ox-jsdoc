import vue from '@vitejs/plugin-vue'
import { createReadStream, readFileSync } from 'node:fs'
import { basename, join } from 'node:path'
import { defineConfig, type Plugin } from 'vite'
import { voidPlugin } from 'void'

const oxcParserBrowserAssetNames = [
  'browser-bundle.js',
  'parser.wasm32-wasi.wasm',
  'wasi-worker-browser.mjs'
]
const oxJsdocWasmAssetNames = ['ox_jsdoc_wasm_bg.wasm']
const oxcParserBrowserAssetDir = join(
  process.cwd(),
  'node_modules',
  '@oxc-parser',
  'binding-wasm32-wasi'
)

function resolveOxcParserBrowserAsset(filename: string): string {
  return join(oxcParserBrowserAssetDir, filename)
}

function getContentType(filename: string): string {
  if (filename.endsWith('.wasm')) {
    return 'application/wasm'
  }

  return 'text/javascript; charset=utf-8'
}

function oxcParserBrowserAssets(): Plugin {
  return {
    name: 'oxc-parser-browser-assets',
    configureServer(server) {
      server.middlewares.use('/vendor/oxc-parser', (request, response, next) => {
        const pathname = decodeURIComponent((request.url ?? '').split('?')[0] ?? '')
        const filename = basename(pathname)

        if (!oxcParserBrowserAssetNames.includes(filename)) {
          next()
          return
        }

        response.setHeader('Content-Type', getContentType(filename))
        createReadStream(resolveOxcParserBrowserAsset(filename)).pipe(response)
      })
    },
    async generateBundle() {
      for (const filename of oxcParserBrowserAssetNames) {
        this.emitFile({
          type: 'asset',
          fileName: `vendor/oxc-parser/${filename}`,
          source: readFileSync(resolveOxcParserBrowserAsset(filename))
        })
      }
    }
  }
}

function oxJsdocWasmAssets(): Plugin {
  const assetDir = join(process.cwd(), '..', '..', 'wasm', 'ox-jsdoc', 'pkg')

  return {
    name: 'ox-jsdoc-wasm-assets',
    configureServer(server) {
      server.middlewares.use('/vendor/ox-jsdoc', (request, response, next) => {
        const pathname = decodeURIComponent((request.url ?? '').split('?')[0] ?? '')
        const filename = basename(pathname)

        if (!oxJsdocWasmAssetNames.includes(filename)) {
          next()
          return
        }

        response.setHeader('Content-Type', getContentType(filename))
        createReadStream(join(assetDir, filename)).pipe(response)
      })
    },
    async generateBundle() {
      for (const filename of oxJsdocWasmAssetNames) {
        this.emitFile({
          type: 'asset',
          fileName: `vendor/ox-jsdoc/${filename}`,
          source: readFileSync(join(assetDir, filename))
        })
      }
    }
  }
}

export default defineConfig({
  build: {
    chunkSizeWarningLimit: 4000
  },
  plugins: [oxcParserBrowserAssets(), oxJsdocWasmAssets(), voidPlugin(), vue()]
})
