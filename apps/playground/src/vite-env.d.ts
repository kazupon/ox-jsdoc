/**
 * Vite environment declarations for the ox-jsdoc playground.
 *
 * @author kazuya kawaguchi (a.k.a. kazupon)
 * @license MIT
 */

/// <reference types="vite/client" />

declare module '*.vue' {
  import type { DefineComponent } from 'vue'

  const component: DefineComponent<Record<string, never>, Record<string, never>, unknown>
  export default component
}

declare module 'monaco-editor/esm/vs/editor/editor.api.js' {
  export * from 'monaco-editor'
}

declare module 'monaco-editor/esm/vs/basic-languages/javascript/javascript.contribution.js' {
  const _module: unknown
  export default _module
}

declare module 'monaco-editor/esm/vs/basic-languages/typescript/typescript.contribution.js' {
  const _module: unknown
  export default _module
}

declare module '*.css' {
  const stylesheet: string
  export default stylesheet
}

declare module 'monaco-editor/esm/vs/editor/editor.worker?worker' {
  const editorWorker: new () => Worker
  export default editorWorker
}

declare const __OX_JS_DOC_WASM_VERSION__: string
