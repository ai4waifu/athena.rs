//! 一次分派后的 limb 视图（不拥有所有权）。

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
