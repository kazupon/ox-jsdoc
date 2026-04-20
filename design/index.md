# ox-jsdoc Design Documents

This directory is the table of contents for the design documents of `ox-jsdoc`.
It collects documents related to syntax, AST design, and performance design.

## Table of Contents

### Core Design

- [Syntax foundation](./syntax.ebnf)
- [AST shape and memory model](./ast.md)

### Performance Design

- [Performance design overview](./001-performance/README.md)
- [Principles and reference points](./001-performance/principles.md)
- [What not to bring in yet](./001-performance/non-goals.md)
- [Implementation guidance](./001-performance/implementation.md)

### Project Structure

- [Repository and workspace layout](./002-project-structure/README.md)

### Binary AST

- [Binary AST overview](./007-binary-ast/README.md)
- [Architecture (背景・アーキテクチャ)](./007-binary-ast/architecture.md)
- [AST Nodes (対象ノード一覧)](./007-binary-ast/ast-nodes.md)
- [Binary Format (バイナリフォーマット)](./007-binary-ast/format.md)
- [Encoding (ツリー・Variant・compat_mode)](./007-binary-ast/encoding.md)
- [JS Decoder (JS lazy decoder)](./007-binary-ast/js-decoder.md)
- [Rust Implementation (Rust 内部実装)](./007-binary-ast/rust-impl.md)
- [Testing Strategy (テスト戦略)](./007-binary-ast/testing.md)
- [Benchmark Strategy (ベンチマーク戦略)](./007-binary-ast/benchmark.md)
- [Phases (実装フェーズ)](./007-binary-ast/phases.md)
- [Batch Processing (batch 5 論点と決定)](./007-binary-ast/batch-processing.md)

### Diagrams

- [AST overview diagram](./ast-example.svg)
- [AST structure diagram](./ast-structure.svg)
- [AST memory layout diagram](./ast-memory-layout.svg)
