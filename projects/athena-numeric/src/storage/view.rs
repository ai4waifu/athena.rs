//! 一次分派后的 limb 视图与 checked magnitude 解码（不拥有所有权）。
#![allow(unsafe_code)]

use athena_types::{Diagnostic, DiagnosticCode};

use super::{
    meta::{Mode, heap_len, try_mode_of},
    union::Magnitude,
};

/// 只读 kernel 视图：ptr + len。
#[derive(Debug, Clone, Copy)]
pub(crate) struct LimbView<'a> {
    limbs: &'a [u64],
}

impl<'a> LimbView<'a> {
    /// 从已存在的 slice 构造（测试 / 适配）。
    #[inline]
    pub(crate) fn from_slice(limbs: &'a [u64]) -> Self {
        Self { limbs }
    }

    /// 借用为 slice。
    #[inline]
    pub(crate) fn as_slice(self) -> &'a [u64] {
        self.limbs
    }

    /// 逻辑长度。
    #[inline]
    pub(crate) fn len(self) -> usize {
        self.limbs.len()
    }
}

impl<'a> AsRef<[u64]> for LimbView<'a> {
    fn as_ref(&self) -> &[u64] {
        self.limbs
    }
}

/// 可写 kernel 视图。
pub(crate) struct MutableLimbView<'a> {
    limbs: &'a mut [u64],
}

impl<'a> MutableLimbView<'a> {
    /// 从可变 slice 构造。
    #[inline]
    pub(crate) fn from_slice(limbs: &'a mut [u64]) -> Self {
        Self { limbs }
    }

    /// 借用为可变 slice。
    #[inline]
    pub(crate) fn as_mut_slice(self) -> &'a mut [u64] {
        self.limbs
    }
}

/// 已校验的 magnitude 视图（Living `19` checked decoder）。
#[derive(Debug, Clone, Copy)]
pub(crate) struct CheckedMagnitudeView<'a> {
    mode: Mode,
    limbs: &'a [u64],
}

impl<'a> CheckedMagnitudeView<'a> {
    /// Mode。
    #[inline]
    pub(crate) fn mode(self) -> Mode {
        self.mode
    }

    /// 只读 limbs。
    #[inline]
    pub(crate) fn limbs(self) -> &'a [u64] {
        self.limbs
    }

    /// Kernel 视图。
    #[inline]
    pub(crate) fn as_limb_view(self) -> LimbView<'a> {
        LimbView::from_slice(self.limbs)
    }
}

/// 集中解码 `meta + Magnitude`（拒绝 reserved mode、非法 Limb2/Heap 形状）。
pub(crate) fn decode_magnitude<'a>(meta: usize, magnitude: &'a Magnitude) -> Result<CheckedMagnitudeView<'a>, Diagnostic> {
    let mode = try_mode_of(meta).map_err(|_| decode_err("magnitude_reserved_mode"))?;
    match mode {
        Mode::Limb1 => {
            // SAFETY: mode 已校验为 Limb1，limb1 为 active field。
            let limbs = unsafe { core::slice::from_ref(&magnitude.limb1) };
            Ok(CheckedMagnitudeView { mode, limbs })
        }
        Mode::Limb2 => {
            // SAFETY: mode Limb2。
            let limbs = unsafe { &magnitude.limb2 };
            if limbs[1] == 0 {
                return Err(decode_err("magnitude_limb2_high_zero"));
            }
            Ok(CheckedMagnitudeView { mode, limbs })
        }
        Mode::Heap => {
            let len = heap_len(meta);
            if len < 3 {
                return Err(decode_err("magnitude_heap_len"));
            }
            // SAFETY: mode Heap。
            let heap = unsafe { magnitude.heap };
            if heap.capacity < len {
                return Err(decode_err("magnitude_heap_capacity"));
            }
            let n = len.min(heap.capacity);
            // SAFETY: n <= capacity。
            let limbs = unsafe { core::slice::from_raw_parts(heap.ptr.as_ptr(), n) };
            if limbs[n - 1] == 0 {
                return Err(decode_err("magnitude_heap_trailing_zero"));
            }
            Ok(CheckedMagnitudeView { mode, limbs })
        }
    }
}

fn decode_err(operation: &str) -> Diagnostic {
    Diagnostic::new(DiagnosticCode::NumericConversionForbidden)
        .detail("domain", "numeric")
        .detail("operation", operation)
}

/// 宽度分类（由 limb 切片导出，等价于 mode 分派）。
#[derive(Debug, Clone, Copy)]
pub(crate) enum LimbWidth<'a> {
    /// 逻辑零。
    Zero,
    /// 单 limb。
    Limb1(u64),
    /// 双 limb（高位非零）。
    Limb2([u64; 2]),
    /// ≥3 limb（借自调用方）。
    Wide(&'a [u64]),
}

impl<'a> LimbWidth<'a> {
    /// 由小端 limbs 分类（自动 trim 尾随零；`[0]` → Zero）。
    #[inline]
    pub(crate) fn classify(limbs: &'a [u64]) -> Self {
        if limbs.is_empty() || crate::kernel::limb::is_zero(limbs) {
            return Self::Zero;
        }
        let n = crate::kernel::limb::effective_len(limbs);
        match n {
            0 => Self::Zero,
            1 => Self::Limb1(limbs[0]),
            2 => Self::Limb2([limbs[0], limbs[1]]),
            _ => Self::Wide(&limbs[..n]),
        }
    }
}
