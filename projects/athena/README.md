# `athena`

`athena` is the public Rust facade for the athena computer algebra kernel. It
composes the workspace contracts, Core IR, and rewrite engine into one
mathematical execution surface.

It owns `athenaEngine` and `Session`, numeric promotion, runtime objects,
symbols and bindings, domain dispatch, evaluation, simplification,
differentiation, integration, and language-neutral `athena_*` diagnostics.
Numerics, matrices, polynomials, number theory, and graph-related mathematics
are modules of this one kernel, not separate `athena-num` or `athena-eval`
products.

## Boundary with SXO

athena does not parse Mathematica or MATLAB and does not provide N-API or
WebAssembly bindings. SXO performs parsing, dialect lowering, rendering,
locale selection, and host integration. SXO adapters must not reimplement
athena's numeric or object semantics.

```text
SXO frontend -> athena IR/value -> athenaEngine -> result/diagnostic
```

```sh
cargo test -p athena
cargo doc -p athena --no-deps
```
