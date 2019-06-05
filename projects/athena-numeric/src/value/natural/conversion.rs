//! [`Natural`] 十进制与 wire 幅度编解码。

use athena_types::{Diagnostic, DiagnosticCode, Result};
use std::str::FromStr;

use super::Natural;
use crate::kernel::limb as limb_kernel;

impl Natural {
    /// 十进制字符串（无符号）。
    ///
    /// Living `19`：只借用 limb 并在本地 `Vec` 上辗转相除，**不** `Clone` / 不登记 root / 不走 GC 分配。
    pub fn to_decimal_string(&self) -> String {
        Self::decimal_from_limbs(self.as_limbs())
    }

    /// 由小端 limb 生成无符号十进制（观察者路径；无 owning 数值复制）。
    pub(crate) fn decimal_from_limbs(limbs: &[u64]) -> String {
        if limbs.is_empty() || limb_kernel::is_zero(limbs) {
            return "0".to_string();
        }
        let mut buf: Vec<u64> = limbs.to_vec();
        let mut digits = Vec::new();
        // `limb_kernel::is_zero` 对空切片为 false；除尽后须保留 `[0]` 或显式判空，避免死循环。
        loop {
            if buf.is_empty() || limb_kernel::is_zero(&buf) {
                break;
            }
            let mut rem = 0u128;
            for limb in buf.iter_mut().rev() {
                let cur = (rem << 64) | u128::from(*limb);
                *limb = (cur / 10) as u64;
                rem = cur % 10;
            }
            digits.push(b'0' + rem as u8);
            while buf.len() > 1 && buf.last() == Some(&0) {
                buf.pop();
            }
        }
        if digits.is_empty() {
            return "0".to_string();
        }
        digits.reverse();
        String::from_utf8(digits).expect("ASCII digit bytes are UTF-8")
    }

    /// 二进制 wire 幅度：`u32` 小端 limb 计数 + `u64` 小端 limb。
    ///
    /// 零编码为 `count=1` 且 limb `0`。解码拒绝 `count=0` 与尾随零 limb。
    pub(crate) fn wire_encode_magnitude(&self) -> Vec<u8> {
        let limbs = self.as_limbs();
        let el = limbs.len();
        let mut out = Vec::with_capacity(4 + el * 8);
        out.extend_from_slice(&(el as u32).to_le_bytes());
        for &limb in limbs {
            out.extend_from_slice(&limb.to_le_bytes());
        }
        out
    }

    /// 在执行预算下解码 [`Self::wire_encode_magnitude`] 字节（canonical reject）。
    pub(crate) fn wire_decode_magnitude_budgeted(bytes: &[u8], budget: &crate::policy::execution_budget::ExecutionBudget) -> Result<Self> {
        use crate::format::validation::assert_canonical_magnitude_limbs;
        if bytes.len() < 4 {
            return Err(Diagnostic::new(DiagnosticCode::NumericConversionForbidden)
                .detail("domain", "numeric")
                .detail("operation", "wire_magnitude_short"));
        }
        budget.check_wire_bytes(bytes.len())?;
        let count = u32::from_le_bytes(bytes[0..4].try_into().map_err(|_| {
            Diagnostic::new(DiagnosticCode::NumericConversionForbidden).detail("domain", "numeric").detail("operation", "wire_magnitude_count")
        })?) as usize;
        budget.check_limbs(count)?;
        let need = 4usize
            .checked_add(count.checked_mul(8).ok_or_else(|| {
                Diagnostic::new(DiagnosticCode::NumericConversionForbidden)
                    .detail("domain", "numeric")
                    .detail("operation", "wire_magnitude_overflow")
            })?)
            .ok_or_else(|| {
                Diagnostic::new(DiagnosticCode::NumericConversionForbidden)
                    .detail("domain", "numeric")
                    .detail("operation", "wire_magnitude_overflow")
            })?;
        if bytes.len() != need {
            return Err(Diagnostic::new(DiagnosticCode::NumericConversionForbidden)
                .detail("domain", "numeric")
                .detail("operation", "wire_magnitude_len"));
        }
        let mut limbs = Vec::with_capacity(count);
        for i in 0..count {
            let off = 4 + i * 8;
            limbs.push(u64::from_le_bytes(bytes[off..off + 8].try_into().map_err(|_| {
                Diagnostic::new(DiagnosticCode::NumericConversionForbidden)
                    .detail("domain", "numeric")
                    .detail("operation", "wire_magnitude_limb")
            })?));
        }
        assert_canonical_magnitude_limbs(count, &limbs)?;
        Self::from_limbs(limbs)
    }

    /// 解码 [`Self::wire_encode_magnitude`] 字节。
    pub(crate) fn wire_decode_magnitude(bytes: &[u8]) -> Result<Self> {
        Self::wire_decode_magnitude_budgeted(bytes, &crate::policy::execution_budget::ExecutionBudget::unlimited())
    }

    /// 从拼接的有理载荷中拆出首个幅度块。
    pub(crate) fn wire_take_magnitude(bytes: &[u8]) -> Result<(Self, &[u8])> {
        if bytes.len() < 4 {
            return Err(Diagnostic::new(DiagnosticCode::NumericConversionForbidden)
                .detail("domain", "numeric")
                .detail("operation", "wire_magnitude_short"));
        }
        let count = u32::from_le_bytes(bytes[0..4].try_into().map_err(|_| {
            Diagnostic::new(DiagnosticCode::NumericConversionForbidden).detail("domain", "numeric").detail("operation", "wire_magnitude_count")
        })?) as usize;
        let total = 4usize
            .checked_add(count.checked_mul(8).ok_or_else(|| {
                Diagnostic::new(DiagnosticCode::NumericConversionForbidden)
                    .detail("domain", "numeric")
                    .detail("operation", "wire_magnitude_overflow")
            })?)
            .ok_or_else(|| {
                Diagnostic::new(DiagnosticCode::NumericConversionForbidden)
                    .detail("domain", "numeric")
                    .detail("operation", "wire_magnitude_overflow")
            })?;
        if bytes.len() < total {
            return Err(Diagnostic::new(DiagnosticCode::NumericConversionForbidden)
                .detail("domain", "numeric")
                .detail("operation", "wire_magnitude_truncated"));
        }
        let mag = Self::wire_decode_magnitude(&bytes[..total])?;
        Ok((mag, &bytes[total..]))
    }
}

impl FromStr for Natural {
    type Err = ();

    /// 十进制解析（仅数字，无符号）。
    fn from_str(digits: &str) -> std::result::Result<Self, Self::Err> {
        if digits.is_empty() {
            return Err(());
        }
        if !digits.chars().all(|c| c.is_ascii_digit()) {
            return Err(());
        }
        let mut n = Self::zero();
        for ch in digits.chars() {
            n = n.mul_u64(10).add_u64(u64::from(ch as u32 - u32::from(b'0')));
        }
        Ok(n)
    }
}
