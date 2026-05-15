#!/bin/bash

set -e

# Restore all git changes
git restore --source=HEAD --staged --worktree -- package.json pnpm-lock.yaml

# Release packages
TAG="latest"

# Public WASM packages allowlist:
# - canonical Binary AST WASM (`@ox-jsdoc/wasm`)
# - thin alias for one deprecation cycle (`@ox-jsdoc/wasm-binary`)
# `wasm/ox-jsdoc-origin` is `"private": true` and is not published.
WASM_PACKAGES=(
  "wasm/ox-jsdoc"
  "wasm/ox-jsdoc-binary"
)

for PKG in "${WASM_PACKAGES[@]}"; do
  pushd "$PKG"
  echo "⚡ Publishing $PKG with tag $TAG"
  pnpm publish --access public --no-git-checks --tag $TAG
  popd > /dev/null
done
