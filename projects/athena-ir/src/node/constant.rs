//! 封闭数学常量（前端中立原子）。
//!
//! 方言表层名（`Pi`、`pi`、`π`、`E`、`e`、`ℯ`）仅在 SXO lowering 映射到此。
//! Athena 执行不得从用户符号显示名反向推断这些常量。

/// 类型化数学常量原子载荷。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MathematicalConstant {
    /// 圆周率常量 π。
    Pi,
    /// 自然对数底 e。
    EulerNumber,
}

impl MathematicalConstant {
    /// 指纹 / wire 用的稳定 discriminant。
    pub const fn discriminant(self) -> u8 {
        match self {
            Self::Pi => 1,
            Self::EulerNumber => 2,
        }
    }

    /// 调试 / 诊断标签（不是方言表层名合同）。
    pub const fn debug_label(self) -> &'static str {
        match self {
            Self::Pi => "Pi",
            Self::EulerNumber => "E",
        }
    }
}
