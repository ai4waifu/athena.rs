//! 默认 portable 宿主合同（WASM 安全、确定性、语义基线）。
//!
//! 与 [`crate::kernel::portable`] 算法实现正交：本模块只广告 capability / wire 上限。

use super::{NumericBackend, NumericBackendContract, NumericBackendLimits, NumericCapability, NumericOperation, NumericResultMode};
use crate::{domain::NumericDomain, precision::PrecisionKind};

const PORTABLE_WIRE_PAYLOAD_LIMIT: u32 = 1 << 20;

const PORTABLE_CAPS: &[NumericCapability] = &[
    NumericCapability::ExactInteger,
    NumericCapability::ExactRational,
    NumericCapability::MachineReal,
    NumericCapability::ArbitraryRealSkeleton,
    NumericCapability::ModularInteger,
    NumericCapability::IntervalEnclosure,
    NumericCapability::DirectedRounding,
    NumericCapability::ExplicitPromotion,
    NumericCapability::Deterministic,
];

const PORTABLE_CONTRACT: NumericBackendContract = NumericBackendContract {
    id: "portable",
    wasm_safe: true,
    native_only: false,
    jit_eligible: true,
    deterministic: true,
    default_radix: 10,
    capabilities: PORTABLE_CAPS,
    limits: NumericBackendLimits {
        max_limbs: None,
        max_significand_bits: Some(53),
        max_wire_payload_bytes: Some(PORTABLE_WIRE_PAYLOAD_LIMIT),
        max_pow_exp: Some(crate::value::integer::Integer::MAX_POW_EXP),
    },
};

/// 解码与序列化守卫用的 wire 载荷字节上限。
pub(crate) const PORTABLE_WIRE_PAYLOAD_LIMIT_BYTES: u32 = PORTABLE_WIRE_PAYLOAD_LIMIT;

/// 默认 portable 宿主合同（非 machine kernel）。
#[derive(Debug, Clone, Copy, Default)]
pub struct PortableBackend;

impl NumericBackend for PortableBackend {
    fn contract(&self) -> &'static NumericBackendContract {
        &PORTABLE_CONTRACT
    }

    fn supports_domain(&self, domain: &NumericDomain) -> bool {
        matches!(
            domain,
            NumericDomain::Integer
                | NumericDomain::Rational
                | NumericDomain::Real
                | NumericDomain::Complex
                | NumericDomain::Interval
                | NumericDomain::Modular { .. }
        )
    }

    fn supports_precision(&self, kind: PrecisionKind) -> bool {
        matches!(kind, PrecisionKind::Exact | PrecisionKind::Machine | PrecisionKind::Arbitrary | PrecisionKind::Interval)
    }

    fn supports_operation(&self, domain: &NumericDomain, op: NumericOperation, result: NumericResultMode) -> bool {
        if !self.supports_domain(domain) {
            return false;
        }
        match (domain, op, result) {
            (
                NumericDomain::Integer,
                NumericOperation::Add
                | NumericOperation::Sub
                | NumericOperation::Mul
                | NumericOperation::Div
                | NumericOperation::Pow
                | NumericOperation::Neg
                | NumericOperation::Abs
                | NumericOperation::Gcd
                | NumericOperation::Compare
                | NumericOperation::Factorial,
                NumericResultMode::Exact,
            ) => true,
            (
                NumericDomain::Rational,
                NumericOperation::Add
                | NumericOperation::Sub
                | NumericOperation::Mul
                | NumericOperation::Div
                | NumericOperation::Pow
                | NumericOperation::Neg
                | NumericOperation::Abs
                | NumericOperation::Compare,
                NumericResultMode::Exact,
            ) => true,
            (NumericDomain::Integer | NumericDomain::Rational, NumericOperation::Promote, NumericResultMode::Machine) => {
                self.has_capability(NumericCapability::ExplicitPromotion)
            }
            (
                NumericDomain::Real,
                NumericOperation::Add
                | NumericOperation::Sub
                | NumericOperation::Mul
                | NumericOperation::Div
                | NumericOperation::Compare
                | NumericOperation::Promote,
                NumericResultMode::Machine,
            ) => true,
            (NumericDomain::Real, NumericOperation::Promote, NumericResultMode::ArbitrarySkeleton) => {
                self.has_capability(NumericCapability::ArbitraryRealSkeleton)
            }
            (NumericDomain::Interval, NumericOperation::IntervalAdd | NumericOperation::IntervalMul, NumericResultMode::IntervalEnclosure) => {
                self.has_capability(NumericCapability::IntervalEnclosure) && self.has_capability(NumericCapability::DirectedRounding)
            }
            (
                NumericDomain::Modular { .. },
                NumericOperation::Add | NumericOperation::Sub | NumericOperation::Mul | NumericOperation::Pow,
                NumericResultMode::Exact,
            ) => self.has_capability(NumericCapability::ModularInteger),
            _ => false,
        }
    }
}
