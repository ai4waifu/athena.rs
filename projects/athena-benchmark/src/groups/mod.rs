//! 分组 fixture 注册。

mod domains;
mod engine;
mod ir;
mod jit;
mod numeric;
mod rewriter;

use crate::fixture::Suite;

/// 构建默认 suite（含各分组种子 / 占位 fixture）。
pub fn default_suite() -> Suite {
    let mut suite = Suite::new();
    numeric::register(&mut suite);
    ir::register(&mut suite);
    rewriter::register(&mut suite);
    engine::register(&mut suite);
    domains::register(&mut suite);
    jit::register(&mut suite);
    suite
}
