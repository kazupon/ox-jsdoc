# ox-jsdoc Performance Design

This directory contains the performance design documents for `ox-jsdoc`.
They are based on ideas observed in the `oxc` project, but they describe
how those ideas should be adapted for `ox-jsdoc`, not copied verbatim.

This set of documents was split out from [`design/index.md`](../index.md)
to keep the design easier to read and evolve.

Related documents:

- [AST shape and memory model](../ast.md)
- [Syntax foundation](../syntax.ebnf)

## Contents

1. [Principles and reference points](./principles.md)
   - Goals
   - Assumptions already fixed
   - Parser / validator / analyzer separation
   - Compact layout
   - Lossless-enough parsing
   - Generated code
   - Fast path / cold path

2. [What not to bring in yet](./non-goals.md)
   - Core design centered on raw transfer
   - Ultra-low-level lexer micro-optimizations
   - Strong semantic graph identity in the core AST
   - Transport complexity inside parser core

3. [Implementation guidance](./implementation.md)
   - Entry point for the implementation guidance documents
   - Links to the split implementation design pages:
     - [Concrete directions](./implementation/directions.md)
     - [Architecture and next steps](./implementation/architecture.md)
     - [Parser API and allocation contract](./implementation/parser-api.md)
     - [Scanner / parser boundary](./implementation/scanner-parser.md)
     - [Performance measurement strategy](./implementation/measurement.md)
     - [Adoption summary and conclusion](./implementation/adoption.md)
