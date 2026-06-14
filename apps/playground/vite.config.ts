import vue from '@vitejs/plugin-vue'
import { createReadStream, readFileSync } from 'node:fs'
import { basename, join } from 'node:path'
import { defineConfig, type Plugin } from 'vite'
import { voidPlugin } from 'void'

const oxJsdocWasmAssetNames = ['ox_jsdoc_wasm_bg.wasm']
const oxcParserBrowserAssetNames = [
  'browser-bundle.js',
  'parser.wasm32-wasi.wasm',
  'wasi-worker-browser.mjs'
]

function getContentType(filename: string): string {
  if (filename.endsWith('.wasm')) {
    return 'application/wasm'
  }

  return 'text/javascript; charset=utf-8'
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

function oxcParserBrowserAssets(): Plugin {
  const assetDir = join(process.cwd(), 'public', 'vendor', 'oxc-parser')

  return {
    name: 'oxc-parser-browser-assets',
    configureServer(server) {
      server.middlewares.use('/__oxc-parser', (request, response, next) => {
        const pathname = decodeURIComponent((request.url ?? '').split('?')[0] ?? '')
        const filename = basename(pathname)

        if (!oxcParserBrowserAssetNames.includes(filename)) {
          next()
          return
        }

        response.setHeader('Content-Type', getContentType(filename))
        createReadStream(join(assetDir, filename)).pipe(response)
      })
    },
    async generateBundle() {
      for (const filename of oxcParserBrowserAssetNames) {
        this.emitFile({
          type: 'asset',
          fileName: `__oxc-parser/${filename}`,
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
  plugins: [oxJsdocWasmAssets(), oxcParserBrowserAssets(), voidPlugin(), vue()]
})
