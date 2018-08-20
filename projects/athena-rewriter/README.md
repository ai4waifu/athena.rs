# `athena-rewriter`

`athena-rewriter` provides reusable transformations over athena's Core CAS IR.
It is separate from the kernel facade so rewrite behavior can be tested and
composed independently.

Responsibilities include canonicalization, rule application, simplification
results, and rewrite diagnostics while preserving IR and source invariants.
It does not parse frontend languages, own a session, render dialects, or
choose locale text. A rewrite must not silently change exactness or numeric
policy.

The primary API is `Rewriter`, `RewriteOptions`, `RewriteResult`, and
`Rewriter::simplify`.

```sh
cargo test -p athena-rewriter
cargo doc -p athena-rewriter --no-deps
```
