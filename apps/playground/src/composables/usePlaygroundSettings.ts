/**
 * Local-storage backed settings for the ox-jsdoc playground.
 *
 * @author kazuya kawaguchi (a.k.a. kazupon)
 * @license MIT
 */

import { reactive, ref, watch } from 'vue'
import type {
  ParserOptions,
  PlaygroundSettings,
  PlaygroundTheme,
  TypeParseMode
} from '../types/playground'

const settingsKey = 'ox-jsdoc.playground.settings'

const defaultSettings: PlaygroundSettings = {
  compatMode: false,
  parseBatch: true,
  parseTypes: true,
  preserveWhitespace: true,
  theme: 'light',
  typeParseMode: 'typescript'
}

function isTypeParseMode(value: unknown): value is TypeParseMode {
  return value === 'jsdoc' || value === 'closure' || value === 'typescript'
}

function isPlaygroundTheme(value: unknown): value is PlaygroundTheme {
  return value === 'dark' || value === 'light'
}

function loadSettings(): PlaygroundSettings {
  if (typeof localStorage === 'undefined') {
    return { ...defaultSettings }
  }

  try {
    const raw = localStorage.getItem(settingsKey)

    if (!raw) {
      return { ...defaultSettings }
    }

    const parsed = JSON.parse(raw) as Partial<PlaygroundSettings>

    return {
      compatMode:
        typeof parsed.compatMode === 'boolean' ? parsed.compatMode : defaultSettings.compatMode,
      parseBatch:
        typeof parsed.parseBatch === 'boolean' ? parsed.parseBatch : defaultSettings.parseBatch,
      parseTypes:
        typeof parsed.parseTypes === 'boolean' ? parsed.parseTypes : defaultSettings.parseTypes,
      preserveWhitespace:
        typeof parsed.preserveWhitespace === 'boolean'
          ? parsed.preserveWhitespace
          : defaultSettings.preserveWhitespace,
      theme: isPlaygroundTheme(parsed.theme) ? parsed.theme : defaultSettings.theme,
      typeParseMode: isTypeParseMode(parsed.typeParseMode)
        ? parsed.typeParseMode
        : defaultSettings.typeParseMode
    }
  } catch {
    return { ...defaultSettings }
  }
}

function saveSettings(options: ParserOptions, theme: PlaygroundTheme): void {
  if (typeof localStorage === 'undefined') {
    return
  }

  try {
    localStorage.setItem(
      settingsKey,
      JSON.stringify({
        compatMode: options.compatMode,
        parseBatch: options.parseBatch,
        parseTypes: options.parseTypes,
        preserveWhitespace: options.preserveWhitespace,
        theme,
        typeParseMode: options.typeParseMode
      } satisfies PlaygroundSettings)
    )
  } catch {
    // Ignore unavailable storage; parser settings still work for the current session.
  }
}

export function usePlaygroundSettings() {
  const settings = loadSettings()
  const theme = ref<PlaygroundTheme>(settings.theme)
  const options = reactive<ParserOptions>({
    compatMode: settings.compatMode,
    parseBatch: settings.parseBatch,
    parseTypes: settings.parseTypes,
    preserveWhitespace: settings.preserveWhitespace,
    typeParseMode: settings.typeParseMode
  })

  const toggleTheme = () => {
    theme.value = theme.value === 'light' ? 'dark' : 'light'
  }

  watch(
    [
      theme,
      () => options.compatMode,
      () => options.parseBatch,
      () => options.parseTypes,
      () => options.preserveWhitespace,
      () => options.typeParseMode
    ],
    () => saveSettings(options, theme.value)
  )

  return {
    options,
    theme,
    toggleTheme
  }
}
