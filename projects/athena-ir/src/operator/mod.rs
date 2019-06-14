//! 算子注册表（结构身份 · 不含方言表面 catalog）。
//!
//! Living `26`：`athena-ir` 只提供 [`OperatorId`] 分配与反查。
//! Mathematica / MATLAB 等表面名由方言 lowering 注入，禁止在此预置方言表面 catalog。

use std::collections::HashMap;

use athena_types::OperatorId;

/// 字符串名 ↔ [`OperatorId`] 双向表。
#[derive(Debug, Clone, Default)]
pub struct OperatorRegistry {
    names: Vec<String>,
    by_name: HashMap<String, OperatorId>,
}

impl OperatorRegistry {
    /// 空注册表。
    pub fn new() -> Self {
        Self::default()
    }

    /// 分配或查找算子 id。
    pub fn intern(&mut self, name: &str) -> OperatorId {
        if let Some(id) = self.by_name.get(name) {
            return *id;
        }
        let id = OperatorId(self.names.len() as u32);
        self.names.push(name.to_string());
        self.by_name.insert(name.to_string(), id);
        id
    }

    /// 查找已有 id（不分配）。
    pub fn lookup(&self, name: &str) -> Option<OperatorId> {
        self.by_name.get(name).copied()
    }

    /// 反查算子名。
    pub fn name(&self, id: OperatorId) -> Option<&str> {
        self.names.get(id.0 as usize).map(String::as_str)
    }

    /// 已注册数量。
    pub fn len(&self) -> usize {
        self.names.len()
    }

    /// 是否为空。
    pub fn is_empty(&self) -> bool {
        self.names.is_empty()
    }
}
