# ox-jsdoc

## Goal

Provide High performance jsdoc parser like oxc project.

## Development

This repository uses Vite+ as the task runner. Install `vp` before running
project tasks:

```sh
curl -fsSL https://vite.plus | bash
```

Rust license headers are checked with the repository xtask:

```sh
vp run lint:headers
```

The first run builds the local `xtask` crate automatically through Cargo.
You can also run the task directly:

```sh
cargo run -p xtask -- headers:check
```
