//! 复数（骨架；不依赖 `num-complex`）。

use crate::execution_budget::NumericContext;
use athena_types::{Diagnostic, DiagnosticCode, Result};

use crate::value::real::Real;

/// 分支策略（与特殊函数 registry 对齐，骨架枚举）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum BranchPolicy {
    /// 主值。
    #[default]
    Principal,
    /// 仅实数。
    RealOnly,
}

/// 复数。
///
/// 不变量：实部与虚部不得为 NaN。机器实数路径提供加减乘与共轭。混合 [`Real::Decimal`] 返回 `UnsupportedOperation`。
#[derive(Debug, PartialEq)]
pub struct Complex {
    /// 实部。
    pub re: Real,
    /// 虚部。
    pub im: Real,
    /// 分支。
    pub branch: BranchPolicy,
}

impl Complex {
    /// Owning 深复制。
    pub fn try_clone_in(&self, ctx: &crate::execution_budget::NumericContext) -> Result<Self> {
        Ok(Self { re: self.re.try_clone_in(ctx)?, im: self.im.try_clone_in(ctx)?, branch: self.branch })
    }

    /// 校验并构造。
    pub fn try_new(re: Real, im: Real, branch: BranchPolicy) -> Result<Self> {
        let v = Self { re, im, branch };
        v.validate()?;
        Ok(v)
    }

    /// 由实部构造（虚部 0）。
    pub fn from_real(re: Real) -> Result<Self> {
        Self::try_new(re, Real::machine(0.0), BranchPolicy::Principal)
    }

    /// 不变量校验。
    pub fn validate(&self) -> Result<()> {
        if self.re.is_nan() || self.im.is_nan() {
            return Err(invalid("complex_nan"));
        }
        Ok(())
    }

    /// 机器实数路径加法。
    pub fn add(&self, other: &Self) -> Result<Self> {
        let re = machine_f64(&self.re, "complex_add")? + machine_f64(&other.re, "complex_add")?;
        let im = machine_f64(&self.im, "complex_add")? + machine_f64(&other.im, "complex_add")?;
        Self::try_new(Real::machine(re), Real::machine(im), merge_branch(self.branch, other.branch))
    }

    /// 机器实数路径减法。
    pub fn sub(&self, other: &Self) -> Result<Self> {
        let re = machine_f64(&self.re, "complex_sub")? - machine_f64(&other.re, "complex_sub")?;
        let im = machine_f64(&self.im, "complex_sub")? - machine_f64(&other.im, "complex_sub")?;
        Self::try_new(Real::machine(re), Real::machine(im), merge_branch(self.branch, other.branch))
    }

    /// 机器实数路径乘法：`(a+bi)(c+di)`。
    pub fn mul(&self, other: &Self) -> Result<Self> {
        let a = machine_f64(&self.re, "complex_mul")?;
        let b = machine_f64(&self.im, "complex_mul")?;
        let c = machine_f64(&other.re, "complex_mul")?;
        let d = machine_f64(&other.im, "complex_mul")?;
        Self::try_new(Real::machine(a * c - b * d), Real::machine(a * d + b * c), merge_branch(self.branch, other.branch))
    }

    /// 取负。
    pub fn neg(&self) -> Result<Self> {
        Self::try_new(neg_real(&self.re)?, neg_real(&self.im)?, self.branch)
    }

    /// 共轭 `a-bi`（分支不变）。
    pub fn conjugate(&self) -> Result<Self> {
        Self::try_new(self.re.try_clone_in(&NumericContext::portable_default())?, neg_real(&self.im)?, self.branch)
    }
}

fn machine_f64(r: &Real, op: &str) -> Result<f64> {
    match r {
        Real::Machine(x) => {
            if x.is_nan() {
                Err(invalid("complex_nan"))
            }
            else {
                Ok(*x)
            }
        }
        Real::Decimal(_) => Err(Diagnostic::new(DiagnosticCode::UnsupportedOperation).detail("domain", "numeric").detail("operation", op)),
    }
}

fn neg_real(r: &Real) -> Result<Real> {
    match r {
        Real::Machine(x) => {
            if x.is_nan() {
                Err(invalid("complex_nan"))
            }
            else {
                Ok(Real::machine(-*x))
            }
        }
        Real::Decimal(_) => {
            Err(Diagnostic::new(DiagnosticCode::UnsupportedOperation).detail("domain", "numeric").detail("operation", "complex_decimal"))
        }
    }
}

fn merge_branch(a: BranchPolicy, b: BranchPolicy) -> BranchPolicy {
    if a == b { a } else { BranchPolicy::Principal }
}

fn invalid(operation: &str) -> Diagnostic {
    Diagnostic::new(DiagnosticCode::NumericConversionForbidden).detail("domain", "numeric").detail("operation", operation)
}
