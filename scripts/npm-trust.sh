#!/bin/bash

set -euo pipefail

REPO="kazupon/ox-jsdoc"
WORKFLOW_FILE="release.yml"

# Allowlist of npm packages that this repository publishes via trusted publishing.
#
# - Canonical packages (always published in normal releases)
# - Thin aliases (`ox-jsdoc-binary`, `@ox-jsdoc/wasm-binary`) kept for one
#   deprecation cycle as JS-only re-exports of the canonical packages
#
# `@ox-jsdoc/binary-binding-*` are NOT in this list: post-cutover the alias
# `ox-jsdoc-binary` is JS-only and never bundles a native binding, so the
# `binary-binding` platform packages have no role on the canonical path. They
# are addressed on the registry via `npm deprecate` instead of being published.
#
# `ox-jsdoc-origin` and `@ox-jsdoc/wasm-origin` are `"private": true` and are
# not published.
PACKAGES=(
  # Canonical packages
  "@ox-jsdoc/decoder"
  "ox-jsdoc"
  "@ox-jsdoc/wasm"
  "@ox-jsdoc/binding-darwin-arm64"
  "@ox-jsdoc/binding-darwin-x64"
  "@ox-jsdoc/binding-linux-x64-gnu"
  "@ox-jsdoc/binding-win32-x64-msvc"
  # Thin alias packages (one deprecation cycle)
  "ox-jsdoc-binary"
  "@ox-jsdoc/wasm-binary"
)

dry_run=false
if [[ "${1:-}" == "--dry-run" ]]; then
  dry_run=true
fi

for package in "${PACKAGES[@]}"; do
  echo "Configuring trusted publishing for ${package}"
  if [[ "${dry_run}" == true ]]; then
    npm trust github "${package}" --repo "${REPO}" --file "${WORKFLOW_FILE}" --yes --dry-run
    echo "Would require 2FA and disallow token publishing for ${package}"
  else
    trust_log=$(mktemp)
    if npm trust github "${package}" --repo "${REPO}" --file "${WORKFLOW_FILE}" --yes 2>"${trust_log}"; then
      rm -f "${trust_log}"
    elif grep -q "E409" "${trust_log}"; then
      cat "${trust_log}" >&2
      rm -f "${trust_log}"
      echo "Trusted publishing already exists for ${package}; continuing"
    else
      cat "${trust_log}" >&2
      rm -f "${trust_log}"
      exit 1
    fi
    npm access set mfa=publish "${package}"
  fi
done
