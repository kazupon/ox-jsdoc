#!/usr/bin/env node
// Analyze a samply profile.json to print self / inclusive time top N.
//
// Resolves frame addresses by combining (1) `nm -n -U` over the binary
// (demangled in one batch via `rustfilt`) and (2) the profile's own
// nativeSymbol names where available. Designed for macOS arm64 release
// binaries built with `CARGO_PROFILE_RELEASE_DEBUG=true`; pass
// `--load-base 0x400000` for typical Linux x86_64.
//
// Usage:
//   node analyze-profile.js \
//     --binary ./target/release/examples/profile_<entry> \
//     --profile /tmp/profile-out/profile.json \
//     [--filter <substring>] \
//     [--top 30] \
//     [--load-base 0x100000000] \
//     [--addr-cap 0x200000] \
//     [--no-demangle]
//
// Without --filter, the "all" ranking is shown. With --filter "your_crate",
// a "filtered" ranking is shown first.

import fs from 'node:fs';
import { execSync } from 'node:child_process';

// ---- argv parsing ---------------------------------------------------------

function parseArgs(argv) {
  const out = {
    binary: null,
    profile: null,
    filter: null,
    top: 30,
    loadBase: 0x100000000,
    addrCap: 0x200000,
    demangle: true,
  };
  for (let i = 2; i < argv.length; i++) {
    const a = argv[i];
    const need = (key) => {
      if (i + 1 >= argv.length) {
        die(`missing value for ${key}`);
      }
      return argv[++i];
    };
    switch (a) {
      case '--binary':
        out.binary = need('--binary');
        break;
      case '--profile':
        out.profile = need('--profile');
        break;
      case '--filter':
        out.filter = need('--filter');
        break;
      case '--top':
        out.top = parseInt(need('--top'), 10);
        break;
      case '--load-base':
        out.loadBase = numericArg(need('--load-base'));
        break;
      case '--addr-cap':
        out.addrCap = numericArg(need('--addr-cap'));
        break;
      case '--no-demangle':
        out.demangle = false;
        break;
      case '-h':
      case '--help':
        usage();
        process.exit(0);
      default:
        die(`unknown arg: ${a}`);
    }
  }
  if (!out.binary || !out.profile) {
    die('--binary and --profile are required');
  }
  return out;
}

function numericArg(s) {
  // Accepts "0x100", "256", "1_000". Number() handles "0x..." natively.
  const v = Number(s.replace(/_/g, ''));
  if (!Number.isFinite(v)) die(`invalid numeric arg: ${s}`);
  return v;
}

function die(msg) {
  process.stderr.write(`error: ${msg}\n`);
  process.stderr.write('run with --help for usage\n');
  process.exit(2);
}

function usage() {
  process.stderr.write(`Usage: node analyze-profile.js --binary <path> --profile <path>
  [--filter <substr>] [--top N] [--load-base 0x...] [--addr-cap 0x...] [--no-demangle]
`);
}

// ---- nm + rustfilt symbol load -------------------------------------------

function loadNmSymbols(binary, loadBase, demangle) {
  const out = execSync(`nm -n -U --defined-only ${shellQuote(binary)}`, {
    encoding: 'utf8',
    stdio: ['ignore', 'pipe', 'ignore'],
    maxBuffer: 64 * 1024 * 1024,
  });
  const lines = out.split('\n');
  const rvas = [];
  const mangled = [];
  for (const line of lines) {
    const trimmed = line.trim();
    if (!trimmed) continue;
    const parts = trimmed.split(/\s+/);
    if (parts.length < 3) continue;
    const addr = parseInt(parts[0], 16);
    if (!Number.isFinite(addr)) continue;
    const rva = addr - loadBase;
    if (rva < 0) continue;
    const name = parts.slice(2).join(' ');
    rvas.push(rva);
    mangled.push(name);
  }

  let names = mangled;
  if (demangle && mangled.length) {
    try {
      const stdout = execSync('rustfilt', {
        input: mangled.join('\n'),
        encoding: 'utf8',
        maxBuffer: 64 * 1024 * 1024,
      });
      names = stdout.split('\n').slice(0, mangled.length);
      // Pad if rustfilt's output is shorter (shouldn't happen).
      while (names.length < mangled.length) names.push(mangled[names.length]);
    } catch {
      // rustfilt missing or failed — keep mangled names.
    }
  }

  // Sort by rva (already nm -n sorted, but be defensive).
  const order = rvas.map((_, i) => i).sort((a, b) => rvas[a] - rvas[b]);
  const sortedRvas = new Array(order.length);
  const sortedNames = new Array(order.length);
  for (let i = 0; i < order.length; i++) {
    sortedRvas[i] = rvas[order[i]];
    sortedNames[i] = names[order[i]];
  }
  return { rvas: sortedRvas, names: sortedNames };
}

function shellQuote(s) {
  // Conservative quoting for paths.
  return `'${s.replace(/'/g, "'\\''")}'`;
}

// ---- address → symbol resolver -------------------------------------------

function makeResolver(syms) {
  const { rvas, names } = syms;
  return function resolve(addr) {
    let lo = 0;
    let hi = rvas.length;
    while (lo < hi) {
      const mid = (lo + hi) >>> 1;
      if (rvas[mid] <= addr) lo = mid + 1;
      else hi = mid;
    }
    if (lo === 0) return null;
    return names[lo - 1];
  };
}

const HASH_RE = /::h[0-9a-f]{16}\b/;
function shorten(name) {
  if (!name) return name;
  return name.replace(HASH_RE, '');
}

// ---- main -----------------------------------------------------------------

function main() {
  const args = parseArgs(process.argv);

  process.stderr.write(`Loading nm symbols from ${args.binary}...\n`);
  const syms = loadNmSymbols(args.binary, args.loadBase, args.demangle);
  process.stderr.write(`Loaded ${syms.rvas.length} symbols\n`);
  const resolve = makeResolver(syms);

  const prof = JSON.parse(fs.readFileSync(args.profile, 'utf8'));
  const threads = prof.threads || [];
  const selfCounts = new Map();
  const inclCounts = new Map();

  for (const t of threads) {
    if (!t.samples) continue;
    const sa = t.stringArray;
    const frameTable = t.frameTable;
    const stackTable = t.stackTable;
    const samples = t.samples;
    const nativeSyms = t.nativeSymbols || {};

    function frameResolve(fi) {
      const addr = frameTable.address[fi];
      if (addr != null && addr >= 0 && addr < args.addrCap) {
        const r = resolve(addr);
        if (r) return shorten(r);
      }
      // Fallback: nativeSymbol name from the profile string array.
      const nsIdx = frameTable.nativeSymbol[fi];
      if (nsIdx != null && nsIdx >= 0 && nativeSyms.name) {
        const nameIdx = nativeSyms.name[nsIdx];
        if (nameIdx >= 0 && nameIdx < sa.length) {
          const n = sa[nameIdx];
          if (n && !n.startsWith('0x')) return shorten(n);
        }
      }
      return `?@${addr != null ? '0x' + addr.toString(16) : '?'}`;
    }

    const stackLen = stackTable.length;
    const stacks = samples.stack;
    const seen = new Set();
    for (let s = 0; s < stacks.length; s++) {
      const stackIdx = stacks[s];
      if (stackIdx == null) continue;
      seen.clear();
      let topFrame = null;
      let cur = stackIdx;
      let steps = 0;
      while (cur != null && cur !== -1 && cur < stackLen && steps < 256) {
        const fi = stackTable.frame[cur];
        const name = frameResolve(fi);
        if (topFrame === null) topFrame = name;
        if (!seen.has(name)) {
          seen.add(name);
          inclCounts.set(name, (inclCounts.get(name) || 0) + 1);
        }
        const prev = stackTable.prefix[cur];
        if (prev === cur || prev == null) break;
        cur = prev;
        steps++;
      }
      if (topFrame !== null) {
        selfCounts.set(topFrame, (selfCounts.get(topFrame) || 0) + 1);
      }
    }
  }

  let total = 0;
  for (const v of selfCounts.values()) total += v;
  process.stdout.write(`Total samples: ${total}\n\n`);

  const matchesFilter = args.filter
    ? (name) => name && name.includes(args.filter)
    : null;

  function printSection(title, items, n, only) {
    process.stdout.write(`=== ${title} ===\n`);
    let shown = 0;
    for (const [name, cnt] of items) {
      if (only && !only(name)) continue;
      const pct = total ? (100 * cnt / total) : 0;
      const short = name.length < 130 ? name : (name.slice(0, 127) + '...');
      process.stdout.write(`  ${pct.toFixed(2).padStart(6)}% (${String(cnt).padStart(5)})  ${short}\n`);
      shown++;
      if (shown >= n) break;
    }
    process.stdout.write('\n');
  }

  const selfItems = [...selfCounts.entries()].sort((a, b) => b[1] - a[1]);
  if (matchesFilter) {
    printSection(`SELF time TOP ${args.top} (filter: '${args.filter}')`, selfItems, args.top, matchesFilter);
  }
  printSection(`SELF time TOP ${args.top} (all)`, selfItems, args.top);

  const inclItems = [...inclCounts.entries()].sort((a, b) => b[1] - a[1]);
  if (matchesFilter) {
    printSection(`INCLUSIVE time TOP ${args.top} (filter: '${args.filter}')`, inclItems, args.top, matchesFilter);
  }
}

main();
