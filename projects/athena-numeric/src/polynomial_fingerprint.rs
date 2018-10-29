//! 稳定多项式指纹（非 Session 局部 TermId）。

/// 极小多项式 / defining polynomial 的 canonical 内容哈希（Phase 0）。
///
/// 后续接 engine 侧 canonical polynomial value；numeric 层不得持有 IR 句柄。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct PolynomialFingerprint(pub u64);

impl PolynomialFingerprint {
    /// 占位指纹（测试 / 骨架）。
    pub const PLACEHOLDER: Self = Self(0);
}
