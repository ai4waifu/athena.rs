//! Group fixture registration.

mod bigint;
mod domains;
mod engine;
mod infra;
mod ir;
mod jit;
mod numeric;
/// Path-segment ownership / GC microbenches（Living `15`/`18`）。
pub mod path;
mod rewriter;

use crate::fixture::Suite;

/// Build the default suite（不含完整 bigint 矩阵；按需用 [`suite_with_bigint`]）。
pub fn default_suite() -> Suite {
    let mut suite = Suite::new();
    numeric::register(&mut suite);
    path::register(&mut suite);
    ir::register(&mut suite);
    rewriter::register(&mut suite);
    engine::register(&mut suite);
    domains::register(&mut suite);
    jit::register(&mut suite);
    infra::register(&mut suite);
    suite
}

/// Build suite including the full bigint compare matrix（eager prepare）。
pub fn suite_with_bigint() -> Suite {
    let mut suite = default_suite();
    bigint::register(&mut suite);
    suite
}
