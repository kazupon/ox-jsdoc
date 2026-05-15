/**
 * @author kazuya kawaguchi (a.k.a. kazupon)
 * @license MIT
 *
 * `bumpp --execute` hook that propagates the new release version into the
 * publishable Rust crates. `bumpp` itself only knows about `package.json`
 * style files, so without this hook a `pnpm run release` would leave
 * `crates/ox_jsdoc/Cargo.toml` (and the matching entry in `Cargo.lock`)
 * pointing at the previous version.
 *
 * Steps performed:
 *   1. Read the freshly-bumped version from root `package.json`.
 *   2. Rewrite the `[package].version` line of each target `Cargo.toml`.
 *   3. Run `cargo check` against each touched crate so `Cargo.lock` picks
 *      up the new version (cargo updates the lockfile transparently).
 *   4. `git add` the Cargo.toml + Cargo.lock so the commit `bumpp` is
 *      about to create includes them alongside the package.json bumps.
 */

import { execSync } from 'node:child_process'
import fs from 'node:fs'
import path from 'node:path'
import { fileURLToPath } from 'node:url'

const repoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..')

// Allowlist of publishable Cargo crates whose version must track the npm
// release series. Internal crates with `publish = false` are intentionally
// left at `0.0.0` and excluded here.
const TARGETS = ['crates/ox_jsdoc/Cargo.toml']

function readJson(absPath) {
  return JSON.parse(fs.readFileSync(absPath, 'utf8'))
}

function bumpCargoToml(absPath, newVersion) {
  const original = fs.readFileSync(absPath, 'utf8')
  // Match the first top-level `version = "..."` line. The `m` flag anchors
  // ^ to line starts so we only match the [package].version line, not e.g.
  // a dependency line `oxc_allocator = { version = "..." }`.
  const updated = original.replace(/^version = "[^"]*"/m, `version = "${newVersion}"`)
  if (original === updated) {
    throw new Error(`no [package].version line matched in ${absPath}`)
  }
  fs.writeFileSync(absPath, updated)
}

function getCargoPackageName(absPath) {
  const content = fs.readFileSync(absPath, 'utf8')
  const match = content.match(/^name = "([^"]+)"/m)
  if (!match) {
    throw new Error(`no [package].name in ${absPath}`)
  }
  return match[1]
}

function main() {
  const newVersion = readJson(path.join(repoRoot, 'package.json')).version
  if (!newVersion) {
    throw new Error('root package.json has no `version` field')
  }
  console.log(`Propagating version ${newVersion} into Cargo crates…`)

  const touchedFiles = []
  const cargoCheckTargets = []

  for (const relPath of TARGETS) {
    const absPath = path.join(repoRoot, relPath)
    bumpCargoToml(absPath, newVersion)
    touchedFiles.push(relPath)
    cargoCheckTargets.push(getCargoPackageName(absPath))
    console.log(`  bumped ${relPath} → ${newVersion}`)
  }

  // Sync Cargo.lock by running `cargo check` against each bumped crate.
  // `cargo check` is the cheapest cargo subcommand that touches the
  // lockfile; release builds happen later via the existing pipeline.
  if (cargoCheckTargets.length > 0) {
    const args = cargoCheckTargets.flatMap(name => ['-p', name]).join(' ')
    console.log(`  syncing Cargo.lock via \`cargo check ${args}\`…`)
    execSync(`cargo check --quiet ${args}`, { cwd: repoRoot, stdio: 'inherit' })
    touchedFiles.push('Cargo.lock')
  }

  // Stage the new files so the commit `bumpp` is about to create includes
  // them. `bumpp` runs `git commit` against the index, so anything we add
  // here ends up in the same release commit as the package.json bumps.
  console.log(`  git add ${touchedFiles.join(' ')}`)
  execSync(`git add ${touchedFiles.map(f => JSON.stringify(f)).join(' ')}`, {
    cwd: repoRoot,
    stdio: 'inherit'
  })
}

main()
