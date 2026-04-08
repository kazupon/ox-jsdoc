# Adoption Summary

Adopt now:

- arena-backed AST
- span-rich nodes
- parser / semantic separation
- node design that respects compact layout
- preservation of enough raw syntax for later validation
- generated code or mechanical checks where invariants matter
- borrowed-slice-first string handling in the parser hot path
- an explicit parser API contract tying `source_text`, `Allocator`, and AST lifetime
- direct AST construction with internal scanner helpers and small checkpoints

Defer:

- raw transfer
- fixed transport ABI
- deep lexer micro-optimization
- heavy semantic graph IDs inside the core AST
- pre-hashed or interned tag/name tokens until benchmarks justify them
- public token / event streams until recovery or benchmark data justifies them

Avoid:

- validating every tag rule inside the parser
- leaking transport-specific constraints into the core AST
- overfitting to `oxc` implementation details that are justified only at JS/TS parser scale
- allocating owned strings on the parser success path when source slices are enough

## Conclusion

`ox-jsdoc` should be **oxc-inspired**, not **oxc-cloned**.

What matters most is the performance philosophy:

- a lean hot path
- clear phase boundaries
- compact memory layout
- mechanical protection for structural invariants

The most specialized implementation techniques should be introduced only when
measurement shows that JSDoc parsing actually needs them.
