/**
 * Composer options used to create a locale-aware message resolver.
 *
 * @remarks
 * This source fixture models a real TypeScript file where JSDoc comments are
 * attached to declarations and then fed to the JSDoc parser from AST comments.
 *
 * @VueI18nSee Intlify Documentation
 * @defaultValue "en"
 */
export interface ComposerOptions {
  /**
   * The locale used when no request-specific locale is available.
   *
   * @default "en"
   */
  fallbackLocale?: string

  /**
   * Locale message records keyed by locale id.
   *
   * @param locale - The active locale id.
   * @returns A flat message dictionary for the locale.
   */
  resolveMessages?: (locale: string) => Record<string, string>
}

/**
 * Create a composer instance for runtime translation.
 *
 * @param options - Composer configuration.
 * @param options.fallbackLocale - Locale used as a fallback.
 * @returns The configured composer instance.
 */
export function createComposer(options: ComposerOptions) {
  const fallbackLocale = options.fallbackLocale ?? 'en'

  return {
    /**
     * Translate a message key.
     *
     * @param key - Message key to resolve.
     * @param locale - Optional locale override.
     * @returns The resolved message or the original key.
     */
    t(key: string, locale = fallbackLocale) {
      return options.resolveMessages?.(locale)[key] ?? key
    }
  }
}
