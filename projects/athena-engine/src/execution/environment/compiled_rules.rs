//! 已编译规则仓（Living `27` · [`CompiledRuleId`] → 模式 / 替换）。

use std::collections::HashMap;

use athena_types::{CompiledRuleId, TermId};

use crate::reasoning::trs::TermPattern;

/// Session 级已编译规则表（`SessionCommand::RegisterRuleDispatch` 引用）。
#[derive(Debug, Default)]
pub struct CompiledRuleStore {
    rules: HashMap<CompiledRuleId, (TermPattern, TermId)>,
    next_id: u32,
}

impl CompiledRuleStore {
    /// 空仓。
    pub fn new() -> Self {
        Self::default()
    }

    /// 登记已编译规则，返回稳定句柄。
    pub fn intern(&mut self, pattern: TermPattern, replacement: TermId) -> CompiledRuleId {
        let id = CompiledRuleId(self.next_id);
        self.next_id = self.next_id.saturating_add(1);
        self.rules.insert(id, (pattern, replacement));
        id
    }

    /// 查已编译规则。
    pub fn get(&self, id: CompiledRuleId) -> Option<&(TermPattern, TermId)> {
        self.rules.get(&id)
    }

    /// 条目数（测试 / 诊断）。
    pub fn len(&self) -> usize {
        self.rules.len()
    }

    /// 是否为空。
    pub fn is_empty(&self) -> bool {
        self.rules.is_empty()
    }

    /// 清空。
    pub fn clear(&mut self) {
        self.rules.clear();
        self.next_id = 0;
    }
}
