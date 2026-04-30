/**
 * Shared `Symbol.for('nodejs.util.inspect.custom')` helper.
 *
 * Returning a plain object whose prototype is set to an empty named class
 * makes `console.log(node)` print the class label (e.g. `JsdocBlock { ... }`)
 * in Node-family runtimes. Same trick as oxc raw transfer.
 *
 * In browsers `Symbol.for('nodejs.util.inspect.custom')` is harmless (the
 * key just becomes another property on the object).
 *
 * @author kazuya kawaguchi (a.k.a. kazupon)
 * @license MIT
 */

export const inspectSymbol = Symbol.for('nodejs.util.inspect.custom')

/** Constructor signature of the dynamically-named debug classes. */
type DebugClass = new () => object

/**
 * Cache of empty named classes used as the inspect prototype, keyed by
 * type name. The class is created lazily on first access so unused types
 * don't pollute the runtime.
 */
const debugClassCache = new Map<string, DebugClass>()

/**
 * Get (or create) the empty class whose name matches `typeName` so that
 * `console.log(node)` shows `TypeName { ... }` instead of `Object { ... }`.
 */
export function debugClass(typeName: string): DebugClass {
  const cached = debugClassCache.get(typeName)
  if (cached !== undefined) {
    return cached
  }
  // `new Function` is the only way to create a class with a dynamic name
  // that the V8 inspector picks up. It runs once per type then is cached.
  // eslint-disable-next-line @typescript-eslint/no-implied-eval -- typeName comes from a closed set of internal AST kind names, never user input.
  const cls = new Function(`return class ${typeName} {}`)() as DebugClass
  debugClassCache.set(typeName, cls)
  return cls
}

/**
 * Build the inspect-payload from a plain JSON object — moves it under
 * the `typeName`-labelled prototype so Node prints the right class name.
 */
export function inspectPayload(jsonObj: object, typeName: string): object {
  return Object.setPrototypeOf(jsonObj, debugClass(typeName).prototype)
}
