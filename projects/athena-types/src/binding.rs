//! 中性绑定与求值策略（Living `27`）。

/// 绑定存放类别。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BindingKind {
    /// 词法绑定。
    Lexical,
    /// 动态绑定。
    Dynamic,
    /// Session 级绑定。
    Session,
    /// 跨 Session 持久绑定。
    Persistent,
    /// 记忆化绑定。
    Memoized,
    /// 进入规则分派表的绑定。
    Dispatch,
}

/// 绑定值何时求值。
///
/// 替代布尔 `delayed`。方言表面名不得进入本枚举。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BindingEvaluationPolicy {
    /// 写入前求值右值。
    EvaluateBeforeStore,
    /// 存储残余项，不在写入时求值。
    StoreResidualTerm,
    /// 读取时求值。
    EvaluateOnRead,
    /// 应用 / 分派时求值。
    EvaluateOnApply,
    /// 首次读取时求值并记忆。
    MemoizeOnFirstRead,
    /// 仅在显式 materialize 请求时求值。
    ExplicitMaterialization,
}
