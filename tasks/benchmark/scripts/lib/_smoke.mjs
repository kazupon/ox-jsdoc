/**
 * Smoke test for measure.mjs — run two trivial bench cases and print
 * the variance metrics so we can sanity-check the helper.
 */

import { compareRobust, fmtDuration } from './measure.mjs'

const results = await compareRobust([
  {
    name: 'noop',
    fn: () => {
      // Empty body
    }
  },
  {
    name: 'sum 0..1000',
    fn: () => {
      let s = 0
      for (let i = 0; i < 1000; i++) s += i
      return s
    }
  }
])

console.log('| Bench | p50 (trimmed) | spread | rounds |')
console.log('|---|---:|---:|---|')
for (const r of results) {
  console.log(
    `| ${r.name} | ${fmtDuration(r.p50)} | ±${r.spread_pct.toFixed(1)}% | ${r.round_p50s.map(fmtDuration).join(', ')} |`
  )
}
