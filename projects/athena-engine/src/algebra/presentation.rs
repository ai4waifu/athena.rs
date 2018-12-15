//! 域与群的具体表示（与数学 descriptor 分离）。

use athena_numeric::Integer;
use athena_types::{ExtensionId, FieldId, PresentationId};

/// 域 presentation 稳定句柄（内容与 [`FieldPresentation`] 一一对应）。
pub type FieldPresentationId = PresentationId;

/// 群 presentation 稳定句柄。
pub type GroupPresentationId = PresentationId;

/// 域的具体表示种类（算法可后补，边界现冻结）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FieldPresentationKind {
    /// 有理数域 ℚ。
    Rationals,
    /// 素域 𝔽_p。
    PrimeField {
        /// 特征素数 p。
        characteristic: Integer,
    },
    /// F_{p^n} 多项式基（不可约模多项式由 `FieldTable` 持有）。
    FiniteFieldPolynomialBasis {
        /// 所属 FieldId。
        field: FieldId,
        /// 扩张次数 n。
        degree: u32,
    },
    /// Q(α) 幂基。
    NumberFieldPowerBasis {
        /// 扩张 id。
        extension: ExtensionId,
        /// 次数。
        degree: u32,
    },
    /// 塔扩张（递归，由 context 解释）。
    NumberFieldTower {
        /// 基域 presentation。
        base: FieldPresentationId,
        /// 扩张 id。
        extension: ExtensionId,
    },
    /// 有理函数域 K(x)。
    RationalFunctionField {
        /// 基域。
        base: FieldId,
    },
    /// 环的分式域。
    QuotientField {
        /// 源环（多项式环等，`RingId` 句柄）。
        ring: athena_types::RingId,
    },
}

/// 不可变域表示对象。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FieldPresentation {
    /// 句柄。
    pub id: FieldPresentationId,
    /// 所属域 id。
    pub field: FieldId,
    /// 表示种类。
    pub kind: FieldPresentationKind,
}

/// 群的具体表示种类。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GroupPresentationKind {
    /// 置换群：度数 n 上对称群子群。
    Permutation {
        /// 作用度数。
        degree: u32,
    },
    /// 乘法表（小群；唯一允许 TableIndex 元素）。
    ExplicitTable {
        /// 阶。
        order: Integer,
    },
    /// 循环群标准 presentation。
    CyclicPresentation {
        /// 阶。
        order: Integer,
    },
    /// 多态 pc 群（后续）。
    Pc,
    /// 矩阵群（后续）。
    Matrix,
    /// fp 群（后续）。
    FinitelyPresented,
    /// 黑盒群（后续）。
    BlackBox,
}

/// 不可变群表示对象。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GroupPresentation {
    /// 句柄。
    pub id: GroupPresentationId,
    /// 所属群 id。
    pub group: athena_types::GroupId,
    /// 表示种类。
    pub kind: GroupPresentationKind,
}
