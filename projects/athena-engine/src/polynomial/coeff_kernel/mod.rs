//! 系数环专用内核（算法入口选一次，内循环不再匹配 [`CoefficientDomain`]）。

mod integer;
mod prime_field;
mod rational;

pub use integer::ZCoeffKernel;
pub use prime_field::{FpBigKernel, FpWordKernel};
pub use rational::QCoeffKernel;

use athena_numeric::Modulus;
use athena_types::{CoefficientRingId, Diagnostic, DiagnosticCode, Result};

use super::{coeff_ring_table::CoeffRingTable, ring::CoefficientDomain};
use prime_field::{FpKernelKind, select_fp_kernel};

/// 已解析的系数环内核（enum dispatch，无 trait object）。
#[derive(Debug)]
pub enum SpecializedCoeffKernel {
    /// ℤ。
    Integer(ZCoeffKernel),
    /// ℚ。
    Rational(QCoeffKernel),
    /// 𝔽_p · `u64` 字路径。
    FpWord(FpWordKernel),
    /// 𝔽_p · 大素数 `Modulus` path。
    FpBig(FpBigKernel),
}

impl SpecializedCoeffKernel {
    /// 由系数域与预计算模数构造（intern 时调用一次）。
    pub(crate) fn build(domain: &CoefficientDomain, prime_modulus: Option<&Modulus>) -> Result<Self> {
        match domain {
            CoefficientDomain::Integer => Ok(Self::Integer(ZCoeffKernel)),
            CoefficientDomain::Rational => Ok(Self::Rational(QCoeffKernel)),
            CoefficientDomain::FiniteField { .. } => {
                let modulus = prime_modulus.cloned().ok_or_else(unsupported_domain)?;
                match select_fp_kernel(modulus)? {
                    FpKernelKind::Word(k) => Ok(Self::FpWord(k)),
                    FpKernelKind::Big(k) => Ok(Self::FpBig(k)),
                }
            }
            _ => Err(unsupported_domain()),
        }
    }

    /// 精确系数内核支持的系数域。
    pub fn supports(domain: &CoefficientDomain) -> bool {
        matches!(domain, CoefficientDomain::Integer | CoefficientDomain::Rational | CoefficientDomain::FiniteField { .. })
    }

    /// 内核标签（测试 / 诊断）。
    pub fn kind_tag(&self) -> &'static str {
        match self {
            Self::Integer(_) => "Z",
            Self::Rational(_) => "Q",
            Self::FpWord(_) => "FpWord",
            Self::FpBig(_) => "FpBig",
        }
    }
}

/// 绑定 [`CoefficientRingId`] 的系数环运算（Session 内快速路径）。
pub struct CoeffRing<'a> {
    kernel: &'a SpecializedCoeffKernel,
}

impl<'a> CoeffRing<'a> {
    /// 由 intern 表解析专用内核。
    pub fn resolve(id: CoefficientRingId, table: &'a CoeffRingTable) -> Result<Self> {
        let kernel = table.kernel(id)?;
        Ok(Self { kernel })
    }

    /// 由环描述符解析（读 [`super::ring::RingDescriptor::coefficient_ring`]）。
    pub fn for_descriptor(coefficient_ring: CoefficientRingId, table: &'a CoeffRingTable) -> Result<Self> {
        Self::resolve(coefficient_ring, table)
    }

    /// 当前专用内核标签。
    pub fn kind_tag(&self) -> &'static str {
        self.kernel.kind_tag()
    }

    /// 系数加法。
    pub fn add(&self, a: athena_numeric::Number, b: athena_numeric::Number) -> Result<athena_numeric::Number> {
        match self.kernel {
            SpecializedCoeffKernel::Integer(k) => k.add(a, b),
            SpecializedCoeffKernel::Rational(k) => k.add(a, b),
            SpecializedCoeffKernel::FpWord(k) => k.add(a, b),
            SpecializedCoeffKernel::FpBig(k) => k.add(a, b),
        }
    }

    /// 系数减法。
    pub fn sub(&self, a: athena_numeric::Number, b: athena_numeric::Number) -> Result<athena_numeric::Number> {
        self.add(a, athena_numeric::neg(b))
    }

    /// 系数乘法。
    pub fn mul(&self, a: athena_numeric::Number, b: athena_numeric::Number) -> Result<athena_numeric::Number> {
        match self.kernel {
            SpecializedCoeffKernel::Integer(k) => k.mul(a, b),
            SpecializedCoeffKernel::Rational(k) => k.mul(a, b),
            SpecializedCoeffKernel::FpWord(k) => k.mul(a, b),
            SpecializedCoeffKernel::FpBig(k) => k.mul(a, b),
        }
    }

    /// 系数取负。
    pub fn neg(&self, a: athena_numeric::Number) -> Result<athena_numeric::Number> {
        match self.kernel {
            SpecializedCoeffKernel::Integer(k) => k.neg(a),
            SpecializedCoeffKernel::Rational(k) => k.neg(a),
            SpecializedCoeffKernel::FpWord(k) => k.neg(a),
            SpecializedCoeffKernel::FpBig(k) => k.neg(a),
        }
    }

    /// 系数域是否为域。
    pub fn is_field(&self) -> bool {
        match self.kernel {
            SpecializedCoeffKernel::Integer(k) => k.is_field(),
            SpecializedCoeffKernel::Rational(k) => k.is_field(),
            SpecializedCoeffKernel::FpWord(k) => k.is_field(),
            SpecializedCoeffKernel::FpBig(k) => k.is_field(),
        }
    }

    /// 域除法。
    pub fn div(&self, a: athena_numeric::Number, b: athena_numeric::Number) -> Result<athena_numeric::Number> {
        match self.kernel {
            SpecializedCoeffKernel::Integer(k) => k.div(a, b),
            SpecializedCoeffKernel::Rational(k) => k.div(a, b),
            SpecializedCoeffKernel::FpWord(k) => k.div(a, b),
            SpecializedCoeffKernel::FpBig(k) => k.div(a, b),
        }
    }

    /// 乘法逆元。
    pub fn inv(&self, a: athena_numeric::Number) -> Result<athena_numeric::Number> {
        self.div(athena_numeric::Number::small_int(1), a)
    }
}

impl CoefficientDomain {
    /// 精确系数内核支持的系数域。
    pub fn is_f3_supported(&self) -> bool {
        SpecializedCoeffKernel::supports(self)
    }
}

fn unsupported_domain() -> Diagnostic {
    Diagnostic::new(DiagnosticCode::UnsupportedOperation)
        .detail("domain", "polynomial")
        .detail("operation", "coeff_domain_unsupported")
}
