//! 组 fixture 注册。

mod bigint;
mod domains;
mod engine;
mod infra;
mod ir;
mod jit;
mod numeric;
/// 路径段所有权 / GC 微基准。
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

/// 构建含完整 bigint 对比矩阵的套件（eager prepare）。
pub fn suite_with_bigint() -> Suite {
    let mut suite = default_suite();
    bigint::register(&mut suite);
    suite
}
