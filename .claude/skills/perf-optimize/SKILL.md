---
name: perf-optimize
description: Profile + bench-driven Rust performance optimization workflow. Use when the user asks to optimize, profile, or benchmark Rust code, identify hot functions, or improve crate performance. Out of scope (handle separately) — data structure / binary format / wire layout redesign.
---

# Rust Performance Optimization

Iterate the loop: **profile → identify hot → apply known pattern → bench**. Each turn is a small, surgical edit verified with criterion.

## Trigger

- The user says "optimize", "profile", "performance", "make it faster", "bench is slow", "where is the bottleneck"
- A structural change just landed and the hot distribution may have shifted (re-profile)

## Out of scope (handle separately)

- Data structure / binary format / wire layout redesign — needs its own design phase, not a profile-driven micro-opt loop
- Algorithm / complexity changes (O(N²) → O(N))
- One-off `--release` build issues unrelated to performance

## Tools assumed installed

- `samply` (`cargo install samply`)
- `rustfilt` (`cargo install rustfilt`)
- `nm` (Xcode CLI tools on macOS / binutils on Linux)
- `node` (≥18; for the bundled `analyze-profile.js` — Node.js builtins only, no npm install)

## High-level workflow

1. **Build a profile binary**: a tight-loop `examples/*.rs` runner that calls the hot entry point on a representative fixture
   - Build with `CARGO_PROFILE_RELEASE_DEBUG=true cargo build --release --example <target>`
2. **Record with samply**:
   - `samply record --rate 5000 --save-only --output /tmp/profile-out/profile.json --unstable-presymbolicate ./target/release/examples/<target>`
3. **Analyze**: run `node scripts/analyze-profile.js` (Node.js, no deps; lives at the repo root) to print self / inclusive time top 30 (demangled)
4. **Pick a hot function** that matches one of the patterns in [references/attack-patterns.md](references/attack-patterns.md)
5. **Implement the pattern** (usually small, surgical)
6. **Re-bench** with criterion; confirm `change: ... (p < 0.05)`

For each phase, see the matching reference doc:

- [references/profile-workflow.md](references/profile-workflow.md) — profile binary template, samply invocation, analyze script usage
- [references/attack-patterns.md](references/attack-patterns.md) — library of micro-optimization patterns (linear→length-bucketed match, byte-loop→memchr, common-value bypass, cross-crate inline, branchless bitmask, recent cache, empty-case skip, write_unaligned, panic-check removal, …)
- [references/bench-workflow.md](references/bench-workflow.md) — criterion bench runner, result interpretation, p-value rules

## Pick-target heuristics

From the profile self-time top 30:

- **Single function ≥ 5% self** → most attackable. Read its source, look for one of the patterns.
- **Many small `intern_*` / `lookup_*` helpers in top 10** → cross-crate inline missing or common-value bypass opportunity.
- **`memchr_aligned` already at 2-3% self** → already SIMD-accelerated; look elsewhere.
- **Large parser function (`parse_*_pratt`, etc.) at 15-20% self** → big opaque blob; sub-profile by splitting into smaller helpers, or accept and move on.

ROI heuristic: `expected_pct_speedup = pattern_typical_speedup × (function_self_time / total)`. Cheap pattern matches first; bigger surgery only after the cheap wins are exhausted.

## After multiple changes

Re-profile. Hot distribution shifts after each significant change. Don't keep attacking the same function past diminishing returns — switch focus.
