#!/bin/bash

set -e

# Restore all git changes
git restore --source=HEAD --staged --worktree -- package.json pnpm-lock.yaml

# Release packages
TAG="latest"

# Release wasm for npm registry
for PKG in wasm/* ; do
  if [[ -d $PKG ]]; then
    pushd $PKG
    echo "⚡ Publishing $PKG with tag $TAG"
    pnpm publish --access public --no-git-checks --tag $TAG
    popd > /dev/null
  fi
done
