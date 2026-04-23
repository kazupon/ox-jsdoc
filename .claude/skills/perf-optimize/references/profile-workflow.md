# Profiling workflow (samply + analyze script)

## 1. Write a tight-loop runner

Place under `examples/profile_<entry>.rs` of the crate you are profiling. The runner must:

- Load a realistic fixture (not a synthetic micro-input)
- Call the hot entry point in a tight loop with anti-DCE consumption of the result
- Print iter count + per-iter duration to stderr so you can spot environmental noise

Template:

```rust
//! Tight loop runner for samply / cargo flamegraph.
//! Usage: ITERS=15000 samply record ./target/release/examples/profile_<entry>

use std::fs;
use std::path::PathBuf;

fn fixture_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent().unwrap().parent().unwrap()
        .join("fixtures/perf/source/<your-fixture>")
}

fn main() {
    let source = fs::read_to_string(fixture_path()).expect("read fixture");
    // … prepare inputs once, outside the loop …
    let inputs = prepare(&source);

    let iters = std::env::var("ITERS").ok().and_then(|s| s.parse().ok()).unwrap_or(15_000usize);

    let start = std::time::Instant::now();
    let mut sink = 0usize;
    for _ in 0..iters {
        let r = your_hot_function(&inputs);
        sink = sink.wrapping_add(r.binary_bytes.len()); // anti-DCE
    }
    let elapsed = start.elapsed();
    eprintln!("{} iters in {:?} → {:?}/iter", iters, elapsed, elapsed / iters as u32);
    std::process::exit((sink & 0xff) as i32 ^ 0);
}
```

Aim for total runtime 3-10 seconds at sampling rate 5000 Hz → 15-50k samples.

## 2. Build with debug info

Required for backtrace symbolication:

```bash
CARGO_PROFILE_RELEASE_DEBUG=true cargo build --release --example profile_<entry> -p <crate>
```

This emits the `.dSYM` (macOS) / `.debug` (Linux) sections embedded in the binary; `nm -n -U` will resolve every symbol.

## 3. Record with samply

```bash
mkdir -p /tmp/profile-out
ITERS=15000 samply record \
  --rate 5000 \
  --save-only \
  --output /tmp/profile-out/profile.json \
  --unstable-presymbolicate \
  ./target/release/examples/profile_<entry>
```

Output:

- `/tmp/profile-out/profile.json` — main profile (Firefox profiler format)
- `/tmp/profile-out/profile.syms.json` — presymbolicate sidecar (sparse; the analyze script falls back to `nm`)

If you skip `--save-only`, samply opens a browser UI — useful for one-off exploration but not scriptable.

## 4. Analyze

```bash
node scripts/analyze-profile.js \
  --binary ./target/release/examples/profile_<entry> \
  --profile /tmp/profile-out/profile.json \
  --filter "<your_crate_name>"
```

Output (truncated):

```
Total samples: 18345

=== SELF time TOP 30 (relevant) ===
   19.19% ( 3520)  parse_batch_to_bytes
   17.48% ( 3206)  parser::type_parse::parse_type_pratt
   16.77% ( 3077)  parser::context::emit_block
   10.73% ( 1969)  parser::scanner::logical_lines
    2.63% (  483)  writer::binary_writer::BinaryWriter::emit_node_record
    …

=== INCLUSIVE time TOP 25 (relevant) ===
  100.00% (18345)  parse_batch_to_bytes
   41.74% ( 7657)  parse_block_into_data
   32.03% ( 5876)  emit_block
   …
```

## 5. Read the result

- **Self time** = where the CPU is when sampled, excluding callees. The function actively executing.
- **Inclusive time** = self + every callee under it. % of program time spent under this function.
- A function with **17% self** is the most attackable single hot — investigate its source for a known pattern.
- A function with **17% inclusive but 1% self** means callees dominate — drill into the inclusive-top-25 to find the real hot.
- The TOP self list is dominated by big parser/emit functions; small helpers like `intern_*` and `lookup_*` rank in the 1-3% band but are often the easiest wins because they match clean patterns.

## Notes / gotchas

- samply on macOS sometimes leaves frames as `?@0xNNNN` because the presymbolicate sidecar is sparse. The analyze script falls back to `nm -n -U` over the binary itself (~1k symbols typical).
- If the profile looks weird (parse_batch_to_bytes 19% self with no callees attributed), the runtime might be dominated by a leaf function that wasn't sampled enough — bump ITERS.
- Sampling at 5000 Hz balances precision vs sampler overhead. 1000 Hz is too coarse; 10000 Hz can perturb the workload itself.
