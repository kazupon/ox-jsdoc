# ox-jsdoc Design Documents

This directory is the table of contents for the design documents of `ox-jsdoc`. It collects documents related to syntax, AST design, and performance design.

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
- [Architecture (background and architecture)](./007-binary-ast/architecture.md)
- [AST Nodes (target node list)](./007-binary-ast/ast-nodes.md)
- [Binary Format (binary format)](./007-binary-ast/format.md)
- [Encoding (tree, Variant, compat_mode)](./007-binary-ast/encoding.md)
- [JS Decoder (JS lazy decoder)](./007-binary-ast/js-decoder.md)
- [Rust Implementation (Rust internal implementation)](./007-binary-ast/rust-impl.md)
- [Testing Strategy (testing strategy)](./007-binary-ast/testing.md)
- [Benchmark Strategy (benchmark strategy)](./007-binary-ast/benchmark.md)
- [Phases (implementation phases)](./007-binary-ast/phases.md)
- [Batch Processing (batch 5 discussion points and decisions)](./007-binary-ast/batch-processing.md)

### JSDoc Linter Benchmark

- [JSDoc linter benchmark design](./009-jsdoc-linter-benchmark/README.md)

### Internal

- [Mainstream Binary AST migration](./010-main-stream-binary/README.md)

### Diagrams

- [AST overview diagram](./ast-example.svg)
- [AST structure diagram](./ast-structure.svg)
- [AST memory layout diagram](./ast-memory-layout.svg)
