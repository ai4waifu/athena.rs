//! Group fixture registration.

mod bigint;
mod domains;
mod engine;
mod infra;
mod ir;
mod jit;
mod numeric;
mod path;
mod rewriter;

use crate::fixture::Suite;

/// Build the default suite (seed / placeholder fixtures per group).
pub fn default_suite() -> Suite {
    let mut suite = Suite::new();
    numeric::register(&mut suite);
    bigint::register(&mut suite);
    path::register(&mut suite);
    ir::register(&mut suite);
    rewriter::register(&mut suite);
    engine::register(&mut suite);
    domains::register(&mut suite);
    jit::register(&mut suite);
    infra::register(&mut suite);
    suite
}
