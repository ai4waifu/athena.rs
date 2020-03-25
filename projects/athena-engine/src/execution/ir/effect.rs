//! Effect 种类与有序 effect 链。

use super::ids::EffectToken;

/// 可观察运行时 effect 的种类。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EffectKind {
    /// 读取 Session / 作用域绑定。
    ReadBinding,
    /// 写入 Session / 作用域绑定。
    WriteBinding,
    /// 进入词法或动态作用域。
    EnterScope,
    /// 退出词法或动态作用域。
    ExitScope,
    /// 调用类型化 provider。
    CallProvider,
    /// 发布到 `ResultStore`。
    PublishResult,
    /// 显式 GC safepoint。
    Safepoint,
    /// 预算检查点。
    BudgetCheck,
    /// 取消检查点。
    CancellationCheck,
}

/// Module effect 链中的一条边。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EffectEdge {
    /// 本边产生 / 消费的 token。
    pub token: EffectToken,
    /// 前驱 token（入口边为 `None`）。
    pub precedes_from: Option<EffectToken>,
    /// Effect 分类。
    pub kind: EffectKind,
}

impl EffectEdge {
    /// 无前驱的入口 effect。
    pub fn entry(token: EffectToken, kind: EffectKind) -> Self {
        Self { token, precedes_from: None, kind }
    }

    /// 有序后继 effect。
    pub fn after(token: EffectToken, precedes_from: EffectToken, kind: EffectKind) -> Self {
        Self { token, precedes_from: Some(precedes_from), kind }
    }
}
