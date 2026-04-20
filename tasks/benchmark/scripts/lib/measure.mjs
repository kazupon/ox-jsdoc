/**
 * Variance-resistant benchmarking helper around mitata's `measure()`.
 *
 * Why a wrapper:
 *
 * - mitata's `bench()` / `group()` / `run()` DSL exposes the per-run
 *   *average* (`stats.avg`), which is sensitive to outliers (kernel
 *   preemption, GC pauses, thermal throttling).
 * - The default `min_samples = 12` and `min_cpu_time = 642 ms` give a
 *   reliable distribution per measurement, but a single measurement can
 *   still drift by ±5 % between back-to-back invocations on a noisy
 *   laptop.
 *
 * What this helper does:
 *
 * 1. Calls `measure()` directly so we can dial up `min_samples` and
 *    `min_cpu_time` for a tighter per-round distribution.
 * 2. Repeats the measurement over N rounds and uses the *median* of each
 *    round (`stats.p50`), which is naturally robust to single-sample
 *    outliers.
 * 3. Drops the best and worst round (trimmed mean) so a single bad run
 *    (background process burst, GC, etc.) does not dominate the answer.
 * 4. Reports the spread between the worst and best round so the caller
 *    can see at a glance whether the result is trustworthy.
 *
 * @author kazuya kawaguchi (a.k.a. kazupon)
 * @license MIT
 */

import { measure } from 'mitata'

const NS_PER_MS = 1_000_000

/**
 * Measure `fn` with hardened settings + multi-round aggregation.
 *
 * @param {() => any} fn
 * @param {{
 *   rounds?: number,
 *   trim?: number,
 *   minSamples?: number,
 *   minCpuTimeMs?: number,
 *   warmupSamples?: number,
 *   gc?: boolean | (() => void),
 * }} [options]
 * @returns {Promise<{
 *   p50: number,
 *   p50_min: number,
 *   p50_max: number,
 *   spread_pct: number,
 *   round_p50s: number[],
 * }>}
 */
export async function measureRobust(fn, options = {}) {
  const {
    rounds = 5,
    trim = 1,
    discardFirst = true,
    minSamples = 15,
    minCpuTimeMs = 800,
    warmupSamples = 5,
    gc = true
  } = options

  const usableRounds = rounds - (discardFirst ? 1 : 0)
  if (usableRounds <= trim * 2) {
    throw new Error(
      `rounds=${rounds} (usable=${usableRounds} after discardFirst=${discardFirst}) ` +
        `must be > 2 * trim=${trim}`
    )
  }

  // mitata's `measure()` expects `opts.gc` to be a function (it gets
  // invoked verbatim in the generated loop). `gc: true` is shorthand for
  // "use the host's exposed GC if available" — usually `globalThis.gc`,
  // exposed via `node --expose-gc`. Falls back to a no-op if absent so we
  // do not crash in environments without it.
  const gcFn =
    gc === true
      ? typeof globalThis.gc === 'function'
        ? globalThis.gc
        : () => {}
      : gc === false
        ? false
        : gc

  if (gc === true && typeof globalThis.gc !== 'function') {
    console.warn(
      '[measureRobust] globalThis.gc unavailable — pass `--expose-gc` to node ' +
        'for tighter results.'
    )
  }

  const measureOpts = {
    min_samples: minSamples,
    min_cpu_time: minCpuTimeMs * NS_PER_MS,
    warmup_samples: warmupSamples,
    gc: gcFn
  }

  const round_p50s = []
  for (let r = 0; r < rounds; r++) {
    const stats = await measure(fn, measureOpts)
    round_p50s.push(stats.p50)
  }

  // Drop the first round outright — even with mitata's per-measurement
  // warmup, the first invocation of a function under V8 is dominated by
  // JIT compilation paths and inline-cache misses that the later rounds
  // do not pay. Treating it as a free "cold-start warmup" reliably tightens
  // the spread on tight micro-benchmarks.
  const usable = discardFirst ? round_p50s.slice(1) : round_p50s
  const sorted = [...usable].sort((a, b) => a - b)
  const trimmed = sorted.slice(trim, sorted.length - trim)
  const mean = trimmed.reduce((a, b) => a + b, 0) / trimmed.length

  const p50_min = sorted[0]
  const p50_max = sorted[sorted.length - 1]
  const spread_pct = ((p50_max - p50_min) / mean) * 100

  return { p50: mean, p50_min, p50_max, spread_pct, round_p50s }
}

/**
 * Run a list of named benches with `measureRobust` and return a sorted
 * comparison table. Bench order is preserved in the returned array; sort
 * the caller side if a different order is wanted.
 *
 * @param {Array<{ name: string, fn: () => any }>} benches
 * @param {Parameters<typeof measureRobust>[1]} [options]
 */
export async function compareRobust(benches, options) {
  const results = []
  for (const b of benches) {
    const r = await measureRobust(b.fn, options)
    results.push({ name: b.name, ...r })
  }
  return results
}

/**
 * Format a duration in nanoseconds as a human-readable string, picking
 * an appropriate unit (ns / µs / ms).
 *
 * @param {number} v
 */
export function fmtDuration(v) {
  if (v >= 1_000_000) return `${(v / 1_000_000).toFixed(3)} ms`
  if (v >= 1_000) return `${(v / 1_000).toFixed(3)} µs`
  return `${v.toFixed(3)} ns`
}
