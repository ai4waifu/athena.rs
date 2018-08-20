# `athena-ir`

`athena-ir` owns athena's Core CAS representation. Frontends lower their own
syntax into this representation; the kernel evaluates and rewrites it.

The IR is arena-based: terms refer to stable IDs instead of recursively
owning every child. This enables structural sharing, deterministic hashing,
verification, and incremental rewrites.

It provides `TermArena`, `TermKind`, `AtomKind`, `TermBuilder`, `SymbolTable`,
`canonical_hash`, and `TermArena::verify`.

The IR is dialect-neutral. `WExpr`, MATLAB forms, and parser trees do not
belong here. New nodes require construction, verification, hashing, and
serialization tests where applicable.

```sh
cargo test -p athena-ir
cargo doc -p athena-ir --no-deps
```
