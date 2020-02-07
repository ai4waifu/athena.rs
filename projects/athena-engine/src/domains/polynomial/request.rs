//! 多项式域请求（Living `28`：输入为 [`PolynomialRef`]，禁止 owning payload）。

use athena_types::RingId;

use super::{factor::PolynomialFactorLimits, groebner::GroebnerLimits, object_ref::PolynomialRef, ring::DivisionPolicy};

/// 多项式域请求 — 骨架变体，算法逐步填充。
#[derive(Debug, Clone, PartialEq)]
pub enum PolynomialRequest {
    /// 规范化（合并同类项、去零）。
    Normalize {
        /// 输入多项式 DomainObject。
        polynomial: PolynomialRef,
    },
    /// 加法。
    Add {
        /// 左。
        lhs: PolynomialRef,
        /// 右。
        rhs: PolynomialRef,
    },
    /// 乘法。
    Mul {
        /// 左。
        lhs: PolynomialRef,
        /// 右。
        rhs: PolynomialRef,
    },
    /// 单变量除法（策略显式）。
    Div {
        /// 被除式。
        dividend: PolynomialRef,
        /// 除式。
        divisor: PolynomialRef,
        /// 除法策略。
        policy: DivisionPolicy,
    },
    /// 单变量 GCD（骨架）。
    Gcd {
        /// 左。
        lhs: PolynomialRef,
        /// 右。
        rhs: PolynomialRef,
    },
    /// 单变量因式分解（完备性合同；[`PolynomialFactorLimits`] 资源上限）。
    Factor {
        /// 待分解多项式。
        polynomial: PolynomialRef,
        /// 资源限制。
        limits: PolynomialFactorLimits,
    },
    /// Gröbner 基（域系数；[`GroebnerLimits`] 资源合同）。
    Groebner {
        /// 理想生成元。
        generators: Vec<PolynomialRef>,
        /// 资源限制。
        limits: GroebnerLimits,
    },
    /// 消元理想（环须 [`super::order::MonomialOrder::Elimination`]）。
    Eliminate {
        /// 理想生成元。
        generators: Vec<PolynomialRef>,
        /// 资源限制。
        limits: GroebnerLimits,
    },
    /// 从 Partial / ResourceLimited frontier 恢复 Buchberger（Living `30` G1）。
    ///
    /// 输入均为 [`PolynomialRef`]；`pending_pairs` 下标相对 `candidates`。
    ResumeGroebner {
        /// 当前候选基。
        candidates: Vec<PolynomialRef>,
        /// 尚未处理的 critical pairs。
        pending_pairs: Vec<(usize, usize)>,
        /// 因基大小上限未能插入的多项式。
        pending_insertion: Option<PolynomialRef>,
        /// 原始输入生成元数量（证书字段）。
        input_generators: usize,
        /// 已消耗的 S-pair 步数。
        prior_s_pair_steps: u32,
        /// 本轮资源限制。
        limits: GroebnerLimits,
    },
    /// ℤ / ℚ 多项式 → 已注册 𝔽_p 环上的模同态像（Living `30` G1）。
    ModularImage {
        /// 源多项式 DomainObject。
        polynomial: PolynomialRef,
        /// 目标有限域多项式环。
        image_ring: RingId,
    },
    /// 从 𝔽_p 环上的像多项式做 Wang 有理重构到 ℤ / ℚ（模数取自像环）。
    ReconstructModular {
        /// 𝔽_p 上的像多项式 DomainObject。
        image: PolynomialRef,
        /// 目标 ℤ 或 ℚ 多项式环。
        target_ring: RingId,
    },
}
