# `athena-types`

`athena-types` is the dependency-light contract crate shared by the athena
workspace. It contains data structures that must have one meaning across the
IR, rewriter, kernel, and host adapters.

## Scope

- exact and inexact numeric representations and domain metadata;
- precision and rounding policy types;
- stable IDs (`TermId`, `NodeId`, `SymbolId`, `OperatorId`, `DomainId`);
- source spans and language-neutral diagnostics;
- serialization and version markers.

This crate defines contracts, not evaluation algorithms. It has no parser,
session, rewrite engine, frontend AST, N-API, or WebAssembly dependency.

Changing a public type affects every athena crate and SXO binding. Prefer
structured fields and explicit enums over semantic strings, and never discard
exactness through an implicit machine-float conversion.

```sh
cargo test -p athena-types
cargo doc -p athena-types --no-deps
```
