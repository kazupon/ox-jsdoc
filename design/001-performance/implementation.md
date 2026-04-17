# Implementation Guidance

This page is the entry point for implementation guidance.
The detailed content is split into smaller documents so each design area can be
read and updated independently.

## Contents

1. [Concrete directions](./implementation/directions.md)
   - Parser / semantic separation
   - Deferred normalization
   - Node role separation
   - Layout checks
   - Serializer shape
   - Borrowed-slice-first strings

2. [Architecture and next steps](./implementation/architecture.md)
   - Near-term architecture
   - Next implementation steps

3. [Parser API and allocation contract](./implementation/parser-api.md)
   - `parse_comment` API shape
   - source lifetime contract
   - span and allocation contracts
   - diagnostics model
   - temporary parser state

4. [Scanner / parser boundary](./implementation/scanner-parser.md)
   - scanner / parser options
   - pros and cons
   - checkpoint contract
   - v1 boundary decision

5. [Performance measurement strategy](./implementation/measurement.md)
   - benchmark buckets
   - comparison baselines
   - fixture strategy
   - benchmark tooling
   - post-measurement design items

6. [Adoption summary and conclusion](./implementation/adoption.md)
   - what to adopt now
   - what to defer
   - what to avoid

7. [Optimization patterns](./implementation/optimization-patterns.md)
   - Optimization patterns adopted in the comment parser
   - Optimization patterns planned for the type parser
   - Design principles and implementation techniques referenced from oxc_parser
