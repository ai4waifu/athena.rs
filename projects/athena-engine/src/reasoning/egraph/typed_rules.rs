//! 用于 E-Graph 饱和的带类型 [`TermPattern`] 规则。
//!
//! 模式匹配 / 替换的所有权在 [`athena_rewriter`]。本模块仅
//! 为引擎局部饱和打包规则（绝不接纳 M-Graph 事实）。

use athena_rewriter::{RewriteRuleId, TermPattern};
use athena_types::TermId;

/// 一条带类型的重写规则（模式 → 替换模板）。
///
/// **不**实现 [`Clone`]。深复制用 [`Self::owning_copy`]。
#[derive(Debug, PartialEq, Eq)]
pub struct TypedRewriteRule {
    /// 规则标识（与 [`athena_rewriter::RuleSet`] 混用时需谨慎共用 id 空间）。
    pub id: RewriteRuleId,
    /// 左侧中性模式。
    pub pattern: TermPattern,
    /// 右侧替换模板（可含已绑定符号）。
    pub replacement: TermId,
    /// 可选的人工调试标签（非分发键）。
    pub debug_label: Option<&'static str>,
}

impl TypedRewriteRule {
    /// Owning 复制（经 [`TermPattern::owning_copy`]）。
    pub fn owning_copy(&self) -> Self {
        Self { id: self.id, pattern: self.pattern.owning_copy(), replacement: self.replacement, debug_label: self.debug_label }
    }
}

/// 单个饱和作用域内有序的带类型重写规则集合。
///
/// **不**实现 [`Clone`]。深复制用 [`Self::owning_copy`]。
#[derive(Debug, Default)]
pub struct TypedRuleSet {
    rules: Vec<TypedRewriteRule>,
    next_id: u32,
}

impl TypedRuleSet {
    /// Owning 复制（经 [`TypedRewriteRule::owning_copy`]）。
    pub fn owning_copy(&self) -> Self {
        Self { rules: self.rules.iter().map(TypedRewriteRule::owning_copy).collect(), next_id: self.next_id }
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

    /// 注册一条带类型模式 → 替换模板。
    pub fn push(&mut self, pattern: TermPattern, replacement: TermId, debug_label: Option<&'static str>) -> RewriteRuleId {
        let id = RewriteRuleId(self.next_id);
        self.next_id = self.next_id.saturating_add(1);
        self.rules.push(TypedRewriteRule { id, pattern, replacement, debug_label });
        id
    }

    /// 按注册顺序迭代规则。
    pub fn iter(&self) -> impl Iterator<Item = &TypedRewriteRule> {
        self.rules.iter()
    }

    /// 按 id 查找。
    pub fn get(&self, id: RewriteRuleId) -> Option<&TypedRewriteRule> {
        self.rules.iter().find(|r| r.id == id)
    }
}
