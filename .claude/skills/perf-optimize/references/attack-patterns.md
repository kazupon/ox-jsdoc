# Attack patterns

Library of micro-optimization patterns proven on Rust crates. Each pattern is small, surgical, and bench-verified. Pick the one that matches the function source code at the top of the profile.

## 1. Linear scan over a small static table → length-bucketed match

When a function does `for entry in TABLE { if entry.key == key { … } }` over a small static array of strings/keys.

**Before** (linear, O(N) per call):

```rust
const TABLE: &[(&str, u32)] = &[("foo", 1), ("bar", 2), ("baz", 3), ...];
fn lookup(key: &str) -> Option<u32> {
    TABLE.iter().find(|(k, _)| *k == key).map(|(_, v)| *v)
}
```

**After** (length-bucketed match — switch on length first, then the few candidates of that length):

```rust
fn lookup(key: &str) -> Option<u32> {
    match key.len() {
        3 => match key {
            "foo" => Some(1),
            "bar" => Some(2),
            "baz" => Some(3),
            _ => None,
        },
        // … other length classes …
        _ => None,
    }
}
```

For 1-byte keys, switch on the byte directly (`match key.as_bytes()[0]`) — even faster.

**Profile signal**: a `lookup_*` function in the self-time top 5-10. Typical win 1-3% per call site.

## 2. Common-value bypass

When a hot caller passes a small, fixed set of values to a generic helper (e.g. `intern("*")` repeatedly).

**Before**:

```rust
let star = writer.intern("*");        // hashes + lookup table walk
let close = writer.intern("*/");       // same
let nl = writer.intern("\n");          // same
```

**After** (skip the generic path entirely):

```rust
const STAR_INDEX: u32 = 2;
const CLOSE_INDEX: u32 = 3;
const LF_INDEX: u32 = 4;

let star = pre_computed_field(STAR_INDEX);
let close = pre_computed_field(CLOSE_INDEX);
let nl = pre_computed_field(LF_INDEX);
```

Variable values that _might_ match a common one fall through to the generic path:

```rust
fn intern_or_common(writer: &mut W, value: &str) -> Field {
    match value {
        "" => pre_computed_field(EMPTY_INDEX),
        "\n" => pre_computed_field(LF_INDEX),
        "\r\n" => pre_computed_field(CRLF_INDEX),
        other => writer.intern(other),  // generic
    }
}
```

**Profile signal**: a generic interner / lookup function in the top 5 self time, called from a small number of code sites with mostly-fixed inputs. Typical win 1-3%.

## 3. Byte loop → memchr / memchr2 / memchr_iter

When a hot loop scans bytes for one or two delimiters.

**Before** (byte-by-byte):

```rust
let mut i = 0;
while i < bytes.len() {
    if bytes[i] == b'\n' { /* found */ break; }
    i += 1;
}
```

**After** (SIMD-accelerated):

```rust
use memchr;
match memchr::memchr(b'\n', bytes) {
    Some(off) => /* found at off */,
    None => /* not found */,
}
```

Variants:

- `memchr::memchr` — single byte
- `memchr::memchr2` / `memchr3` — 2 / 3 candidates simultaneously (e.g. `{` / `}` for brace matching)
- `memchr::memchr_iter` — iterate all matches; faster than `iter().enumerate().filter()`

**UTF-8 safety**: searching for ASCII bytes (`< 0x80`) is safe inside UTF-8 because continuation bytes are always `>= 0x80` and lead bytes have specific high bits — your delimiter byte never appears mid-character.

**Profile signal**: `<core::slice as Iterator>::find` / `next` in self-time, or a custom byte-scan loop visible as raw assembly. Typical win 5-15% on text-heavy workloads.

## 4. Cross-crate `#[inline]` hint

LLVM does **not** automatically inline `pub fn` across crate boundaries even at `-O3` (no LTO). A small wrapper function called from another crate must be marked explicitly:

**Before**:

```rust
// In crate A
pub fn intern_or_lookup(value: &str) -> Field { … 5 lines … }

// In crate B
intern_or_lookup(name);  // costs a real call instruction
```

**After**:

```rust
// In crate A
#[inline]
pub fn intern_or_lookup(value: &str) -> Field { … }
```

For very-hot trivial wrappers, `#[inline(always)]` may pay off — verify with bench, don't guess.

**Profile signal**: a small wrapper function (1-3 source lines) appearing in self-time top 10-20 with an unexpectedly high call count. Typical win 5-10% on cross-crate hot paths.

**Don't overuse**: `#[inline]` increases compile time and binary size. Apply selectively.

## 5. Branchless bitmask construction

When building a bitmask from booleans:

**Before** (branchy):

```rust
let mut bm = 0u8;
if cond1 { bm |= 0b001; }
if cond2 { bm |= 0b010; }
if cond3 { bm |= 0b100; }
```

**After** (no branches):

```rust
let bm: u8 = (cond1 as u8)
    | ((cond2 as u8) << 1)
    | ((cond3 as u8) << 2);
```

Smaller code, no branch mispredict on a hot path.

**Profile signal**: not directly — apply opportunistically when reviewing hot functions.

## 6. 1-slot recent-call cache

When the same key is interned/looked up many times in a row (e.g. interning the same string repeatedly within a block):

```rust
struct Interner {
    last_key_ptr: *const u8,
    last_key_len: usize,
    last_value: V,
    map: HashMap<String, V>,
}

#[inline]
fn intern(&mut self, key: &str) -> V {
    // Pointer-based equality is one compare; pointer + length identifies
    // the same source slice without a string compare.
    if key.as_ptr() == self.last_key_ptr && key.len() == self.last_key_len {
        return self.last_value;
    }
    let v = self.map.entry(key.to_owned()).or_insert_with(...);
    self.last_key_ptr = key.as_ptr();
    self.last_key_len = key.len();
    self.last_value = v;
    v
}
```

**Profile signal**: HashMap lookup function in the top 5-10 self time, with the same caller in a loop.

**Caveat**: the pointer cache is correct only when keys are stable (sub-slice of a long-lived buffer). For owned `String`s, only length cache works — measure first.

## 7. Empty-case skip

When a generic helper is called even when the input is empty.

**Before**:

```rust
let mut list = writer.begin_list(slot);   // 1 fn call
for child in children {                   // 0 iterations possible
    let i = emit(child);
    writer.record_list_child(&mut list, i);
}
writer.finalize_list(list);               // 1 fn call (writes 0,0)
```

**After**:

```rust
if !children.is_empty() {
    let mut list = writer.begin_list(slot);
    for child in children {
        let i = emit(child);
        writer.record_list_child(&mut list, i);
    }
    writer.finalize_list(list);
}
```

Skips two function calls per empty list. Very effective when empty cases are common (often 50-90% in real fixtures, e.g. inline tags in JSDoc blocks).

**Profile signal**: a small helper function appearing in inclusive top 10 with disproportionate call count vs visible work.

## 8. Direct buffer write with `write_unaligned`

When emitting a fixed-size record into a `Vec<u8>` repeatedly.

**Before** (stack build + memcpy):

```rust
let record = NodeRecord { kind, common_data, … , next_sibling: 0 };
let bytes: [u8; 24] = unsafe { std::mem::transmute(record) };
buf.extend_from_slice(&bytes);   // memcpy
```

**After** (single pointer write into reserved capacity):

```rust
let cur_len = buf.len();
buf.reserve(NODE_RECORD_SIZE);
unsafe {
    let dst = buf.as_mut_ptr().add(cur_len) as *mut NodeRecord;
    dst.write_unaligned(record);
    buf.set_len(cur_len + NODE_RECORD_SIZE);
}
```

Saves the intermediate stack copy. Verified ~12% speedup on tight emit paths.

**Safety**: requires `#[repr(C)]` on the record struct + a `const _: () = assert!(size_of::<NodeRecord>() == NODE_RECORD_SIZE)` static check.

## 9. Drop the panic check (`try_from(...).unwrap()` → `as` cast)

When the surrounding precondition guarantees the value fits.

**Before**:

```rust
let off = u32::try_from(byte_offset).unwrap();  // panic check on hot path
```

**After**:

```rust
// Encoder precondition: source offsets fit in u32 (see format spec).
let off = byte_offset as u32;
```

Always document the precondition in a comment. Use `debug_assert!` for dev builds:

```rust
debug_assert!(byte_offset <= u32::MAX as usize);
```

## 10. `Vec::resize(_, 0)` instead of `extend(repeat_n(0, n))`

`Vec::resize(_, 0)` lowers to `memset` for `Vec<u8>`. The iterator-based `extend(repeat_n(0u8, n))` retains generic dispatch and is measurably slower.

```rust
buf.resize(new_len, 0);  // memset
// vs
buf.extend(std::iter::repeat_n(0u8, n));  // slower
```

## 11. Pre-computed lookup tables in static memory

When a helper repeatedly computes a small fixed mapping (e.g. character classification):

```rust
// Compute once, reuse forever
static IS_DIGIT: [bool; 256] = {
    let mut t = [false; 256];
    let mut i = b'0';
    while i <= b'9' { t[i as usize] = true; i += 1; }
    t
};

#[inline]
fn is_digit(b: u8) -> bool { IS_DIGIT[b as usize] }
```

256-byte table fits in a single L1 cache line group; lookup is a single load.

## 12. Pointer-based source-slice detection

When a function receives an `&str` that _might_ be a sub-slice of a known buffer (vs. an owned/synthesized string), pointer arithmetic identifies which:

```rust
fn intern_source_or_owned(&mut self, value: &str) -> Field {
    let source_start = self.source_ptr as usize;
    let source_end = source_start + self.source_len;
    let value_ptr = value.as_ptr() as usize;
    if source_start != 0
        && value_ptr >= source_start
        && value_ptr + value.len() <= source_end
    {
        // Sub-slice of source — register an offsets-only entry, no copy.
        let off = (value_ptr - source_start) as u32;
        return self.zero_copy_intern(off, off + value.len() as u32);
    }
    // Owned/synthesized — fall back to the regular path.
    self.intern_owned(value)
}
```

Pointer comparison never dereferences either pointer, so it's safe even under racing mutation (which can't happen mid-borrow anyway).

## How to pick a pattern

For each candidate function from the profile self-time top 30:

1. Read the source. Identify the structure:
   - `for ... if eq` over a small table → **#1 length-bucketed match**
   - Generic helper called with mostly-fixed args → **#2 common-value bypass**
   - Byte-by-byte scan → **#3 memchr**
   - Small `pub fn` called from another crate → **#4 cross-crate inline**
   - Repeated calls with the same key → **#6 recent cache**
   - Generic helper called for empty input → **#7 empty-case skip**
   - `extend_from_slice` of fixed-size struct in a loop → **#8 write_unaligned**
   - `try_from(...).unwrap()` on the hot path → **#9 panic check removal**

2. Estimate ROI: `expected_pct = pattern_typical_speedup × (function_self_time / total)`

3. Apply the pattern (1-50 lines of code), bench, and verify the change matches the estimate within an order of magnitude.

If the change is larger than expected: investigate (compiler did extra work). If smaller: noise / the pattern doesn't fit. Move on.

## Don't waste time on

- Functions < 1% self that don't sit on a hot path
- `memchr_aligned` / `core::*` already-SIMD'd helpers — they are the floor
- Allocator hot paths (`alloc::raw_vec::*`) — fix the allocation pattern (Vec::with_capacity, arena reuse) instead
- Anything in `std::backtrace` / `gimli` — those are debug-info parsing, not your code
