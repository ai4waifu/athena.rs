//! 向 E-Graph 候选搜索供料的类型化重写规则。
//!
//! 此处规则为 **内部** 合同 — 不是方言 `ReplacementRule` / Blank 模式。
//! 发出匹配绝不准入 M-Graph 事实。
//!
//! 模式匹配与替换位于本 crate（[`crate::TermPattern`]、
//! [`crate::match_pattern`]、[`crate::substitute`]）。饱和 / 准入由 engine 拥有。

use athena_types::TermId;

/// [`RuleSet`] 内稳定的重写规则标识。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct RewriteRuleId(pub u32);

/// 一条已编译重写规则（模式 → 替换模板）。
///
/// 引导载荷以宿主 [`TermId`] 根保留，供结构 [`RuleSet`] 匹配。
/// 类型化 [`crate::TermPattern`] 规则由本处拥有，供 engine E-Graph 饱和消费。
///
/// 纯 `Copy` 句柄载荷。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RewriteRule {
    /// 规则标识。
    pub id: RewriteRuleId,
    /// 左侧模式 term 根（在匹配器落地前仅结构）。
    pub pattern: TermId,
    /// 右侧替换模板根。
    pub replacement: TermId,
    /// 可选人工调试标签（不是分发键）。
    pub debug_label: Option<&'static str>,
}

/// 单个饱和作用域内的有序重写规则集。
///
/// **不**实现 [`Clone`]。深复制用 [`Self::owning_copy`]。
#[derive(Debug, Default)]
pub struct RuleSet {
    rules: Vec<RewriteRule>,
    next_id: u32,
}

impl RuleSet {
    /// Owning 复制。
    pub fn owning_copy(&self) -> Self {
        Self { rules: self.rules.clone(), next_id: self.next_id }
    }

    /// 空集合。
    pub fn new() -> Self {
        Self::default()
    }

    /// 规则数量。
    pub fn len(&self) -> usize {
        self.rules.len()
    }

    /// 是否为空。
    pub fn is_empty(&self) -> bool {
        self.rules.is_empty()
    }

    /// 注册一对模式 → 替换。
    pub fn push(&mut self, pattern: TermId, replacement: TermId, debug_label: Option<&'static str>) -> RewriteRuleId {
        let id = RewriteRuleId(self.next_id);
        self.next_id = self.next_id.saturating_add(1);
        self.rules.push(RewriteRule { id, pattern, replacement, debug_label });
        id
    }

    /// 按注册顺序迭代规则。
    pub fn iter(&self) -> impl Iterator<Item = &RewriteRule> {
        self.rules.iter()
    }

    /// 按 id 查找。
    pub fn get(&self, id: RewriteRuleId) -> Option<&RewriteRule> {
        self.rules.iter().find(|r| r.id == id)
    }
}

/// 局部重写见证（条件 / 溯源稍后填充）。
///
/// 纯 `Copy` 句柄载荷。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LocalRewriteWitness {
    /// 触发的规则。
    pub rule: RewriteRuleId,
    /// 匹配到的主体 term。
    pub subject: TermId,
    /// 产生的 term。
    pub produced: TermId,
}
