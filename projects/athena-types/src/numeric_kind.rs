//! 数值种类与轻量 descriptor（实现在 `athena-numeric`）。

/// 数值种类（合同层描述，非算术实现）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum NumericKind {
    /// 整数。
    Integer,
    /// 有理。
    Rational,
    /// 实。
    Real,
    /// 复。
    Complex,
    /// 区间。
    Interval,
    /// 代数。
    Algebraic,
    /// 有限域。
    FiniteField,
    /// 模整数。
    Modular,
    /// p-adic。
    PAdic,
}

/// 数值类型注册 id。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct NumericTypeId(pub u32);

/// 精度策略 id。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct PrecisionPolicyId(pub u32);

/// 模数对象 id（Session 句柄；值类型在 numeric）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ModulusId(pub u32);
