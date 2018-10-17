//! 多项式域请求。

use super::{expr::Polynomial, groebner::GroebnerLimits, ring::DivisionPolicy};

/// 多项式域请求 — 骨架变体，算法逐步填充。
#[derive(Debug, Clone, PartialEq)]
pub enum PolynomialRequest {
    /// 规范化（合并同类项、去零）。
    Normalize {
        /// 输入多项式。
        polynomial: Polynomial,
    },
    /// 加法。
    Add {
        /// 左。
        lhs: Polynomial,
        /// 右。
        rhs: Polynomial,
    },
    /// 乘法。
    Mul {
        /// 左。
        lhs: Polynomial,
        /// 右。
        rhs: Polynomial,
    },
    /// 单变量除法（策略显式）。
    Div {
        /// 被除式。
        dividend: Polynomial,
        /// 除式。
        divisor: Polynomial,
        /// 除法策略。
        policy: DivisionPolicy,
    },
    /// 单变量 GCD（骨架）。
    Gcd {
        /// 左。
        lhs: Polynomial,
        /// 右。
        rhs: Polynomial,
    },
    /// 因式分解（骨架）。
    Factor {
        /// 待分解多项式。
        polynomial: Polynomial,
    },
    /// Gröbner 基（域系数；[`GroebnerLimits`] 资源合同）。
    Groebner {
        /// 理想生成元。
        generators: Vec<Polynomial>,
        /// 资源限制。
        limits: GroebnerLimits,
    },
    /// 消元理想（环须 [`super::order::MonomialOrder::Elimination`]）。
    Eliminate {
        /// 理想生成元。
        generators: Vec<Polynomial>,
        /// 资源限制。
        limits: GroebnerLimits,
    },
}
