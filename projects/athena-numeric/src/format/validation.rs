//! ANV1 / wire 规范校验（非规范编码应拒绝，而非静默归一化）。

use athena_types::{Diagnostic, DiagnosticCode};

/// 规范 wire 拒绝原因（稳定 `reason` 字符串）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WireReject {
    /// 幅度 `count == 0`（规范零为 `count=1` 且 limb `0`）。
    MagCountZero,
    /// 高位 limb 为 0（尾随零）。
    MagTrailingZero,
    /// `sign=0` 但幅度非零。
    SignZeroNonzeroMag,
    /// `sign=1` 但幅度为零。
    SignPosZeroMag,
    /// `sign=2` 但幅度为零。
    SignNegZeroMag,
    /// 未知符号码。
    SignUnknown,
    /// 有理分母为零。
    RationalDenomZero,
    /// 有理分母符号非正（载荷幅度路径不应出现负分母语义）。
    RationalDenomSign,
    /// 有理未既约。
    RationalUnreduced,
    /// 零有理的分母不是 1。
    RationalZeroDenomNotOne,
    /// 有理载荷尾随字节。
    RationalTrailing,
    /// Real 未知 subtype。
    RealUnknownSubtype,
    /// Real Machine 载荷长度非法（须恰好 8 字节 IEEE bits）。
    RealMachineLen,
    /// Real Machine 为 NaN（未定义特殊值）。
    RealMachineNan,
    /// Real Decimal 精度为 0。
    RealDecimalPrecisionZero,
    /// Real Decimal 尾数宽于声明精度。
    RealDecimalPrecisionExceeds,
    /// Real Decimal 尾数未规范（非零且偶）。
    RealDecimalNotNormalized,
    /// Real Decimal 载荷尾随字节。
    RealDecimalTrailing,
    /// Real 零指数非 0。
    RealDecimalZeroExp,
    /// Interval 未知 subtype。
    IntervalUnknownSubtype,
    /// Interval 未知 decoration。
    IntervalUnknownDecoration,
    /// Interval Bounded 尾随字节或截断。
    IntervalTrailing,
    /// Interval Bounded 端点非法（NaN / 倒置）。
    IntervalBadBounds,
    /// Modular 仅支持嵌入模数（拒绝 intern id）。
    ModularInterned,
    /// Modular 模数非法（`≤ 1`）。
    ModularBadModulus,
    /// Modular 剩余未约化（`residue ≥ modulus`）。
    ModularResidueUnreduced,
    /// Modular 载荷尾随字节。
    ModularTrailing,
    /// Complex 载荷尾随字节或截断。
    ComplexTrailing,
    /// Complex 未知分支码。
    ComplexUnknownBranch,
    /// Algebraic 未知 subtype。
    AlgebraicUnknownSubtype,
    /// Algebraic 载荷截断或尾随。
    AlgebraicTrailing,
    /// Algebraic 占位与非零指纹 / 根下标冲突。
    AlgebraicPlaceholder,
    /// Algebraic 指纹与表示不一致或隔离区间为空。
    AlgebraicInconsistent,
    /// FiniteField 系数为空。
    FiniteFieldEmpty,
    /// FiniteField 载荷截断或尾随。
    FiniteFieldTrailing,
    /// PAdic 素数非法。
    PAdicBadPrime,
    /// PAdic 精度为 0。
    PAdicPrecisionZero,
    /// PAdic 位数超过精度。
    PAdicDigitsLen,
    /// PAdic digit 超出 `p`。
    PAdicDigitRange,
    /// PAdic 高位 digit 为 0（未规范化）。
    PAdicUnnormalized,
    /// PAdic 载荷截断或尾随。
    PAdicTrailing,
}

impl WireReject {
    /// 稳定 reason 标签。
    pub fn as_str(self) -> &'static str {
        match self {
            Self::MagCountZero => "mag_count_zero",
            Self::MagTrailingZero => "mag_trailing_zero",
            Self::SignZeroNonzeroMag => "sign_zero_nonzero_mag",
            Self::SignPosZeroMag => "sign_pos_zero_mag",
            Self::SignNegZeroMag => "sign_neg_zero_mag",
            Self::SignUnknown => "sign_unknown",
            Self::RationalDenomZero => "rational_denom_zero",
            Self::RationalDenomSign => "rational_denom_sign",
            Self::RationalUnreduced => "rational_unreduced",
            Self::RationalZeroDenomNotOne => "rational_zero_denom_not_one",
            Self::RationalTrailing => "rational_trailing",
            Self::RealUnknownSubtype => "real_unknown_subtype",
            Self::RealMachineLen => "real_machine_len",
            Self::RealMachineNan => "real_machine_nan",
            Self::RealDecimalPrecisionZero => "real_decimal_precision_zero",
            Self::RealDecimalPrecisionExceeds => "real_decimal_precision_exceeds",
            Self::RealDecimalNotNormalized => "real_decimal_not_normalized",
            Self::RealDecimalTrailing => "real_decimal_trailing",
            Self::RealDecimalZeroExp => "real_decimal_zero_exp",
            Self::IntervalUnknownSubtype => "interval_unknown_subtype",
            Self::IntervalUnknownDecoration => "interval_unknown_decoration",
            Self::IntervalTrailing => "interval_trailing",
            Self::IntervalBadBounds => "interval_bad_bounds",
            Self::ModularInterned => "modular_interned",
            Self::ModularBadModulus => "modular_bad_modulus",
            Self::ModularResidueUnreduced => "modular_residue_unreduced",
            Self::ModularTrailing => "modular_trailing",
            Self::ComplexTrailing => "complex_trailing",
            Self::ComplexUnknownBranch => "complex_unknown_branch",
            Self::AlgebraicUnknownSubtype => "algebraic_unknown_subtype",
            Self::AlgebraicTrailing => "algebraic_trailing",
            Self::AlgebraicPlaceholder => "algebraic_placeholder",
            Self::AlgebraicInconsistent => "algebraic_inconsistent",
            Self::FiniteFieldEmpty => "finite_field_empty",
            Self::FiniteFieldTrailing => "finite_field_trailing",
            Self::PAdicBadPrime => "padic_bad_prime",
            Self::PAdicPrecisionZero => "padic_precision_zero",
            Self::PAdicDigitsLen => "padic_digits_len",
            Self::PAdicDigitRange => "padic_digit_range",
            Self::PAdicUnnormalized => "padic_unnormalized",
            Self::PAdicTrailing => "padic_trailing",
        }
    }
}

/// 构造 ANV1 非规范拒绝诊断。
pub fn reject_non_canonical(reason: WireReject) -> Diagnostic {
    reject_non_canonical_reason(reason.as_str())
}

/// 拒绝非规范零 / 高位零 / 非法符号等。
pub fn reject_non_canonical_reason(reason: &str) -> Diagnostic {
    Diagnostic::new(DiagnosticCode::NumericConversionForbidden)
        .detail("domain", "numeric")
        .detail("wire", "ANV1")
        .detail("reason", reason)
}

/// 校验 ANV1 幅度 limb 已规范：`count≠0`；`count≥2` 时最高 limb ≠ 0。
pub fn assert_canonical_magnitude_limbs(count: usize, limbs: &[u64]) -> Result<(), Diagnostic> {
    if count == 0 {
        return Err(reject_non_canonical(WireReject::MagCountZero));
    }
    if limbs.len() != count {
        return Err(reject_non_canonical_reason("mag_len_mismatch"));
    }
    if count >= 2 && limbs[count - 1] == 0 {
        return Err(reject_non_canonical(WireReject::MagTrailingZero));
    }
    Ok(())
}
