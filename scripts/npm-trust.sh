#!/bin/bash

set -euo pipefail

REPO="kazupon/ox-jsdoc"
WORKFLOW_FILE="release.yml"

PACKAGES=(
  "@ox-jsdoc/decoder"
  "@ox-jsdoc/wasm"
  "@ox-jsdoc/wasm-binary"
  "ox-jsdoc"
  "ox-jsdoc-binary"
  "@ox-jsdoc/binding-darwin-arm64"
  "@ox-jsdoc/binding-darwin-x64"
  "@ox-jsdoc/binding-linux-x64-gnu"
  "@ox-jsdoc/binding-win32-x64-msvc"
  "@ox-jsdoc/binary-binding-darwin-arm64"
  "@ox-jsdoc/binary-binding-darwin-x64"
  "@ox-jsdoc/binary-binding-linux-x64-gnu"
  "@ox-jsdoc/binary-binding-win32-x64-msvc"
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
