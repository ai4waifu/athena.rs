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
