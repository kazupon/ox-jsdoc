/**
 * Shared types for the ox-jsdoc playground.
 *
 * @author kazuya kawaguchi (a.k.a. kazupon)
 * @license MIT
 */

export type TypeParseMode = 'jsdoc' | 'closure' | 'typescript'

export type PlaygroundTheme = 'light' | 'dark'

export type SourceRange = [number, number]

export type ParserOptions = {
  compatMode: boolean
  parseBatch: boolean
  parseTypes: boolean
  preserveWhitespace: boolean
  typeParseMode: TypeParseMode
}

export type PlaygroundSettings = ParserOptions & {
  theme: PlaygroundTheme
}

export type OxcComment = {
  end?: number
  range?: SourceRange
  start?: number
}

export type OxcParserModule = {
  parseSync(
    filename: string,
    sourceText: string,
    options?: {
      range?: boolean
      sourceType?: 'module' | 'script' | 'commonjs' | 'unambiguous'
    }
  ): {
    comments?: OxcComment[]
    errors?: Array<{ message?: string }>
  }
}

export type ParsedJsdocComment = {
  ast: unknown
  baseOffset: number
  range: SourceRange
  sourceText: string
  type: 'JsdocComment'
}

export type ParsedJsdocSourceFile = {
  comments: ParsedJsdocComment[]
  range: SourceRange
  type: 'JsdocSourceFile'
}

export type ParseView =
  | {
      status: 'loading' | 'error'
      ast: null
      diagnostics: string[]
      duration: null
      tagCount: 0
      inlineTagCount: 0
    }
  | {
      status: 'ok' | 'invalid' | 'error'
      ast: unknown
      diagnostics: string[]
      duration: number
      tagCount: number
      inlineTagCount: number
    }

export type AstSelection = {
  path: string
  range: SourceRange | null
  value: unknown
}
