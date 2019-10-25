//! 多项式域请求（Living `28`：输入为 [`PolynomialRef`]，禁止 owning payload）。

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
}
