//! 系数环专用内核（算法入口选一次，内循环不再匹配 [`CoefficientDomain`]）。

mod integer;
mod prime_field;
mod rational;

pub use integer::ZCoefficientKernel;
pub use prime_field::{FpBigKernel, FpWordKernel};
pub use rational::QCoefficientKernel;

use athena_numeric::Modulus;
use athena_types::{CoefficientRingId, Diagnostic, DiagnosticCode, Result};

use super::{coefficient_ring_table::CoefficientRingTable, ring::CoefficientDomain};
use crate::runtime::values::numeric_clone::clone_modulus;
use prime_field::{FpKernelKind, select_fp_kernel};

/// 已解析的系数环内核（enum dispatch，无 trait object）。
#[derive(Debug)]
pub enum SpecializedCoefficientKernel {
    /// ℤ。
    Integer(ZCoefficientKernel),
    /// ℚ。
    Rational(QCoefficientKernel),
    /// 𝔽_p · `u64` 字路径。
    FpWord(FpWordKernel),
    /// 𝔽_p · 大素数 `Modulus` path。
    FpBig(FpBigKernel),
}

impl SpecializedCoefficientKernel {
    /// 由系数域与预计算模数构造（intern 时调用一次）。
    pub(crate) fn build(domain: &CoefficientDomain, prime_modulus: Option<&Modulus>) -> Result<Self> {
        match domain {
            CoefficientDomain::Integer => Ok(Self::Integer(ZCoefficientKernel)),
            CoefficientDomain::Rational => Ok(Self::Rational(QCoefficientKernel)),
            CoefficientDomain::FiniteField { .. } => {
                let modulus = prime_modulus.map(clone_modulus).ok_or_else(unsupported_domain)?;
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
pub struct CoefficientRing<'a> {
    kernel: &'a SpecializedCoefficientKernel,
}

impl<'a> CoefficientRing<'a> {
    /// 由 intern 表解析专用内核。
    pub fn resolve(id: CoefficientRingId, table: &'a CoefficientRingTable) -> Result<Self> {
        let kernel = table.kernel(id)?;
        Ok(Self { kernel })
    }

    /// 由环描述符解析（读 [`super::ring::RingDescriptor::coefficient_ring`]）。
    pub fn for_descriptor(coefficient_ring: CoefficientRingId, table: &'a CoefficientRingTable) -> Result<Self> {
        Self::resolve(coefficient_ring, table)
    }

    /// 当前专用内核标签。
    pub fn kind_tag(&self) -> &'static str {
        self.kernel.kind_tag()
    }

    /// 系数加法。
    pub fn add(&self, a: athena_numeric::Number, b: athena_numeric::Number) -> Result<athena_numeric::Number> {
        match self.kernel {
            SpecializedCoefficientKernel::Integer(k) => k.add(a, b),
            SpecializedCoefficientKernel::Rational(k) => k.add(a, b),
            SpecializedCoefficientKernel::FpWord(k) => k.add(a, b),
            SpecializedCoefficientKernel::FpBig(k) => k.add(a, b),
        }
    }

    /// 系数减法。
    pub fn sub(&self, a: athena_numeric::Number, b: athena_numeric::Number) -> Result<athena_numeric::Number> {
        self.add(a, athena_numeric::neg(b))
    }

    /// 系数乘法。
    pub fn mul(&self, a: athena_numeric::Number, b: athena_numeric::Number) -> Result<athena_numeric::Number> {
        match self.kernel {
            SpecializedCoefficientKernel::Integer(k) => k.mul(a, b),
            SpecializedCoefficientKernel::Rational(k) => k.mul(a, b),
            SpecializedCoefficientKernel::FpWord(k) => k.mul(a, b),
            SpecializedCoefficientKernel::FpBig(k) => k.mul(a, b),
        }
    }

    /// 系数取负。
    pub fn neg(&self, a: athena_numeric::Number) -> Result<athena_numeric::Number> {
        match self.kernel {
            SpecializedCoefficientKernel::Integer(k) => k.neg(a),
            SpecializedCoefficientKernel::Rational(k) => k.neg(a),
            SpecializedCoefficientKernel::FpWord(k) => k.neg(a),
            SpecializedCoefficientKernel::FpBig(k) => k.neg(a),
        }
    }

    /// 系数域是否为域。
    pub fn is_field(&self) -> bool {
        match self.kernel {
            SpecializedCoefficientKernel::Integer(k) => k.is_field(),
            SpecializedCoefficientKernel::Rational(k) => k.is_field(),
            SpecializedCoefficientKernel::FpWord(k) => k.is_field(),
            SpecializedCoefficientKernel::FpBig(k) => k.is_field(),
        }
    }

    /// 域除法。
    pub fn div(&self, a: athena_numeric::Number, b: athena_numeric::Number) -> Result<athena_numeric::Number> {
        match self.kernel {
            SpecializedCoefficientKernel::Integer(k) => k.div(a, b),
            SpecializedCoefficientKernel::Rational(k) => k.div(a, b),
            SpecializedCoefficientKernel::FpWord(k) => k.div(a, b),
            SpecializedCoefficientKernel::FpBig(k) => k.div(a, b),
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
        SpecializedCoefficientKernel::supports(self)
    }
}

fn unsupported_domain() -> Diagnostic {
    Diagnostic::new(DiagnosticCode::UnsupportedOperation).detail("domain", "polynomial").detail("operation", "coefficient_domain_unsupported")
}
