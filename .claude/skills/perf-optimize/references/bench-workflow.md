# Benchmark workflow (criterion)

## Run a single bench

```bash
cargo bench -p <crate> --bench <bench_name> 2>&1 | tail -50
```

The first run establishes a baseline. Subsequent runs compare against the saved baseline and print the change.

## Output format

```
phase 1+2 — parse + emit (no finish, full file)
                        time:   [220.86 µs 222.77 µs 225.15 µs]
                        change: [-2.6203% -1.6679% -0.6796%] (p = 0.00 < 0.05)
                        Performance has improved.
```

Three numbers per line:

- **time**: lower / mean / upper bound of the 95% CI of this run's per-iteration time
- **change**: lower / mean / upper of the 95% CI of the change vs the baseline
- **(p = X < 0.05)**: probability the change is due to noise; < 0.05 = statistically significant

## Interpretation

| Output line                                           | Meaning                                              |
| ----------------------------------------------------- | ---------------------------------------------------- |
| `Performance has improved.` and `(p < 0.05)`          | Real improvement                                     |
| `Performance has regressed.` and `(p < 0.05)`         | Real regression — investigate or revert              |
| `No change in performance detected.` and `(p > 0.05)` | Likely no effect, or change is below noise floor     |
| `Change within noise threshold.`                      | Borderline; consider re-running with more iterations |

The mean (middle number of `change:`) is what to quote when reporting the result:

- `change: [-2.6203% -1.6679% -0.6796%]` → quote `-1.7%` (round to 1 dp)

## Outliers

```
Found 12 outliers among 100 measurements (12.00%)
  3 (3.00%) low mild
  8 (8.00%) high mild
  1 (1.00%) high severe
```

- < 5% mild outliers — fine
- > 10% outliers — environment noise. Close other apps, disable Spotlight indexing, run on AC power
- "high severe" — usually a thermal throttle event; re-run

## Baseline management

After a major refactor, the saved baseline becomes stale. Reset:

```bash
cargo bench -p <crate> --bench <bench_name> -- --save-baseline new
# … later …
cargo bench -p <crate> --bench <bench_name> -- --baseline new
```

For day-to-day micro-opt loop, just run plain `cargo bench` — criterion saves the previous result automatically.

## Best practices

- **Run on AC power**, no other heavy processes
- **Pin to performance cores** if available (macOS: `taskpolicy -c utility … cargo bench …` to _avoid_ perf cores; default is fine)
- **Re-establish baseline** after a major refactor
- **For short benchmarks** (< 10 µs/iter), use `--measurement-time 30` to reduce noise
- **Don't trust a single 1-run delta** under 1% — re-run

## Selecting which bench to run

Run only the bench(es) the change is expected to affect; saves time and reduces p-hacking risk.

For typical Rust crates:

- `parser` bench — parse-side hot paths
- `encoder` / `serializer` bench — write-side
- `decoder` / `reader` bench — read-side
- end-to-end bench — full pipeline (most representative for user-facing perf)

If the bench takes > 60 s and you're iterating quickly, scope down with criterion's filter:

```bash
cargo bench -p <crate> --bench <bench_name> -- "phase 1+2"
```
