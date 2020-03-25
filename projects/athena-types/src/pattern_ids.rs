//! 模式与值类型约束句柄（· 禁止字符串 head 约束）。

/// 值类型身份（模式 / 守卫用，非方言表面名）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ValueTypeId {
    /// 精确整数。
    ExactInteger,
    /// 符号原子。
    Symbol,
    /// 字符串原子。
    String,
    /// 布尔原子。
    Boolean,
    /// 空值原子。
    Null,
    /// 任意数值塔成员（细分类由 numeric kind 扩展）。
    Numeric,
}

/// 谓词注册句柄（Session / registry 本地）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct PredicateId(pub u32);

/// 规则分派表句柄。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct DispatchTableId(pub u32);

/// 已编译规则句柄。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct CompiledRuleId(pub u32);
