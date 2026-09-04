//! `MatrixParent` — 元素域、shape 策略、精度与稀疏合同（禁止仅用 dtype）。

/// 矩阵元素所属代数 parent（当前精确/机器子集）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ElementParentKind {
    /// 精确整数环 `ℤ`。
    Integers,
    /// 精确有理域 `ℚ`。
    Rationals,
    /// 机器实数（IEEE binary64）。
    MachineReal,
}

impl ElementParentKind {
    /// 是否为精确（无舍入）路径。
    pub const fn is_exact(self) -> bool {
        matches!(self, Self::Integers | Self::Rationals)
    }

    /// 是否为机器数值路径。
    pub const fn is_machine(self) -> bool {
        matches!(self, Self::MachineReal)
    }
}

/// Shape 可变性策略。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ShapePolicy {
    /// 构造后行列固定。
    Fixed,
    /// 允许后续扩展（方言层可变矩阵；内核仍显式 reshape）。
    Dynamic,
}

/// 舍入 / 精度合同。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RoundingPolicy {
    /// 精确路径：禁止静默舍入。
    Exact,
    /// IEEE binary64 机器运算。
    IeeeBinary64,
}

/// 稀疏存储策略（当前仅 Dense；CSR/CSC 后续）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SparseStrategy {
    /// 稠密缓冲。
    Dense,
}

/// 矩阵 parent：元素域 + shape/精度/稀疏策略。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct MatrixParent {
    /// 元素 parent。
    pub element: ElementParentKind,
    /// Shape 策略。
    pub shape_policy: ShapePolicy,
    /// 舍入合同。
    pub rounding: RoundingPolicy,
    /// 稀疏策略。
    pub sparse: SparseStrategy,
}

impl MatrixParent {
    /// 精确整数稠密矩阵。
    pub const fn integers() -> Self {
        Self {
            element: ElementParentKind::Integers,
            shape_policy: ShapePolicy::Fixed,
            rounding: RoundingPolicy::Exact,
            sparse: SparseStrategy::Dense,
        }
    }

    /// 精确有理稠密矩阵。
    pub const fn rationals() -> Self {
        Self {
            element: ElementParentKind::Rationals,
            shape_policy: ShapePolicy::Fixed,
            rounding: RoundingPolicy::Exact,
            sparse: SparseStrategy::Dense,
        }
    }

    /// 机器实数稠密矩阵。
    pub const fn machine_real() -> Self {
        Self {
            element: ElementParentKind::MachineReal,
            shape_policy: ShapePolicy::Fixed,
            rounding: RoundingPolicy::IeeeBinary64,
            sparse: SparseStrategy::Dense,
        }
    }

    /// 与另一 parent 是否可共享同一 buffer 语义（精确与机器永不共享）。
    pub fn buffer_compatible_with(self, other: Self) -> bool {
        self.element == other.element && self.rounding == other.rounding && self.sparse == other.sparse
    }
}
