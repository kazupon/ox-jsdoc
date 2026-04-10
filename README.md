# ox-jsdoc

High-performance JSDoc parser inspired by the `oxc` project.

## Status

> [!WARNING]
> This project is still WIP, so don't use in production.

## Development

This repository uses Vite+ as the task runner. Install `vp` before running the
project tasks:

```sh
curl -fsSL https://vite.plus | bash
```

Common commands:

```sh
vpr fmt       # or `vp run fmt`, format for Rust and JavaScript codes
vpr check     # or `vp run check`, lint for Rust and JavaScript codes
vpr test      # or `vp run test`, test for Rust and JavaScript codes
```

`vpr check` runs the Rust license-header task and `cargo check`.
The header task checks Rust sources for:

- non-empty `@author`
- `@license MIT`

The first run builds the local `xtask` crate automatically through Cargo. You
can also run the task directly:

```sh
cargo run -p xtask -- headers:check
```

Rust commands can be run directly as well:

```sh
cargo fmt --check
cargo check
cargo test
```

## Sponsors

The development of ox-jsdoc is supported by my OSS sponsors!

<p align="center">
  <a href="https://cdn.jsdelivr.net/gh/kazupon/sponsors/sponsors.svg">
    <img alt="sponsor" src="https://cdn.jsdelivr.net/gh/kazupon/sponsors/sponsors.svg">
  </a>
</p>

## License

[MIT](http://opensource.org/licenses/MIT)
