#!/usr/bin/env bash
#
# JSDoc linter benchmark — pure shell driver invoking hyperfine directly.
#
# Modeled after `oxc-project/bench-linter` to keep the per-command launch
# path free of any Node.js / spawnSync wrapper overhead.
#
# Two fixtures × 5 patterns × 1 rule set (combined) = 10 measurements:
#
#   js — refers/eslint-plugin-jsdoc/src/      (.js, espree default parser)
#   ts — refers/vscode/src/                   (.ts, @typescript-eslint/parser)
#
# Patterns (5):
#   1. eslint-jsdoc-upstream         — eslint + upstream eslint-plugin-jsdoc
#   2. oxlint-jsdoc-native           — oxlint built-in JSDoc plugin (Rust)
#   3. eslint-ox-jsdoc-single        — eslint + @ox-jsdoc/eslint-plugin-jsdoc, oxParseStrategy=single
#   4. eslint-ox-jsdoc-batch         — eslint + @ox-jsdoc/eslint-plugin-jsdoc, oxParseStrategy=batch
#   5. oxlint-ox-jsdoc-batch         — oxlint + JS plugin alias `jsdoc-js` + oxParseStrategy=batch
#
# Rule set (1, combined):
#   jsdoc/empty-tags + jsdoc/require-param-description + jsdoc/require-param-type
#   実用的な「JSDoc lint 一式」の代表値、per-rule の breakdown は省略
#
# Usage:
#   1. node tasks/benchmark/scripts/jsdoc-linter-setup.mjs   # generate configs
#   2. bash tasks/benchmark/scripts/jsdoc-linter-hyperfine.sh
#   3. node tasks/benchmark/scripts/jsdoc-linter-report.mjs  # aggregate report

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
BENCH_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
REPO_ROOT="$(cd "$BENCH_ROOT/../.." && pwd)"

FIXTURE_JS="$REPO_ROOT/refers/eslint-plugin-jsdoc/src"
FIXTURE_TS="$REPO_ROOT/refers/vscode/src"
TMP="$BENCH_ROOT/.tmp/jsdoc-linter"
RESULTS="$BENCH_ROOT/results"
ESLINT="$BENCH_ROOT/node_modules/eslint/bin/eslint.js"
OXLINT="$BENCH_ROOT/node_modules/oxlint/bin/oxlint"

WARMUP=1
RUNS=10

OXLINT_FLAGS=(
  --disable-nested-config
  --disable-unicorn-plugin
  --disable-oxc-plugin
  --disable-typescript-plugin
)

# Sanity checks --------------------------------------------------------------

if ! command -v hyperfine >/dev/null 2>&1; then
  echo "error: hyperfine not found in PATH" >&2
  exit 1
fi

if [[ ! -d "$TMP/js" ]] || [[ ! -d "$TMP/ts" ]]; then
  echo "error: configs not generated. Run first:" >&2
  echo "  node $BENCH_ROOT/scripts/jsdoc-linter-setup.mjs" >&2
  exit 1
fi

for f in "$FIXTURE_JS" "$FIXTURE_TS"; do
  if [[ ! -d "$f" ]]; then
    echo "error: fixture missing: $f" >&2
    echo "  hint: 'git submodule update --init refers/...'" >&2
    exit 1
  fi
done

mkdir -p "$RESULTS"

# Per-rule-set runner --------------------------------------------------------

# Each command is wrapped in `cd "$FIXTURE" && CMD` because hyperfine has
# no per-command cwd flag and the lint targets use `.` so they resolve
# inside the fixture directory.
run_set() {
  local fixture_name="$1"
  local fixture_dir="$2"
  local rs="$3"

  local cfg_root="$TMP/$fixture_name"
  local eslint_upstream_cfg="$cfg_root/eslint-jsdoc-upstream/$rs/eslint.config.js"
  local oxlint_native_cfg="$cfg_root/oxlint-jsdoc-native/$rs/.oxlintrc.json"
  local eslint_single_cfg="$cfg_root/eslint-ox-jsdoc-single/$rs/eslint.config.js"
  local eslint_batch_cfg="$cfg_root/eslint-ox-jsdoc-batch/$rs/eslint.config.js"
  local oxlint_batch_cfg="$cfg_root/oxlint-ox-jsdoc-batch/$rs/.oxlintrc.json"

  echo
  echo "=== fixture: $fixture_name | rule set: $rs ==="
  hyperfine \
    --warmup "$WARMUP" \
    --runs "$RUNS" \
    --ignore-failure \
    --export-markdown "$RESULTS/jsdoc-linter-hyperfine-$fixture_name-$rs.md" \
    --export-json "$RESULTS/jsdoc-linter-hyperfine-$fixture_name-$rs.json" \
    --command-name "eslint-jsdoc-upstream" \
      "cd '$fixture_dir' && '$ESLINT' --config '$eslint_upstream_cfg' --no-config-lookup --no-warn-ignored ." \
    --command-name "oxlint-jsdoc-native" \
      "cd '$fixture_dir' && '$OXLINT' --config '$oxlint_native_cfg' ${OXLINT_FLAGS[*]} ." \
    --command-name "eslint-ox-jsdoc-single" \
      "cd '$fixture_dir' && '$ESLINT' --config '$eslint_single_cfg' --no-config-lookup --no-warn-ignored ." \
    --command-name "eslint-ox-jsdoc-batch" \
      "cd '$fixture_dir' && '$ESLINT' --config '$eslint_batch_cfg' --no-config-lookup --no-warn-ignored ." \
    --command-name "oxlint-ox-jsdoc-batch" \
      "cd '$fixture_dir' && '$OXLINT' --config '$oxlint_batch_cfg' ${OXLINT_FLAGS[*]} ."
}

# Run all fixture × rule set combinations -----------------------------------

run_set "js" "$FIXTURE_JS" "combined"
run_set "ts" "$FIXTURE_TS" "combined"

echo
echo "Done. Aggregate into a single report:"
echo "  node $BENCH_ROOT/scripts/jsdoc-linter-report.mjs"
