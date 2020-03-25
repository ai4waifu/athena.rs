//! IR 应用的算子标识。
//!
//! 核心数学 / 逻辑 / 结构算子使用封闭枚举 [`SemanticOperator`]。
//! [`ExtensionRegistry`] **仅为扩展显示名 registry** — 不是
//! 核心语义目录。方言表层名由产品层注入。

mod semantic;

use std::collections::HashMap;

use athena_types::ExtensionOperatorId;

pub use semantic::{ApplicationHead, SemanticOperator, UnaryFunction};

/// 扩展显示名 ↔ [`ExtensionOperatorId`] 双向表。
///
/// 不是核心算子目录。核心算子使用 [`SemanticOperator`]。
///
/// **不**实现 [`Clone`]。深复制用 [`Self::owning_copy`]。
#[derive(Debug, Default)]
pub struct ExtensionRegistry {
    names: Vec<String>,
    by_name: HashMap<String, ExtensionOperatorId>,
}

impl ExtensionRegistry {
    /// 空 registry。
    pub fn new() -> Self {
        Self::default()
    }

    /// Owning 复制（显示名表）。
    pub fn owning_copy(&self) -> Self {
        Self { names: self.names.clone(), by_name: self.by_name.clone() }
    }

    /// 分配或查找扩展算子 id。
    pub fn intern(&mut self, name: &str) -> ExtensionOperatorId {
        if let Some(id) = self.by_name.get(name) {
            return *id;
        }
        let id = ExtensionOperatorId(self.names.len() as u32);
        self.names.push(name.to_string());
        self.by_name.insert(name.to_string(), id);
        id
    }

    /// 解析扩展算子显示名（调试 / 渲染 / 诊断）。
    ///
    /// 切勿用返回字符串还原 [`super::SemanticOperator`]，也勿用其
    /// 选择 executor / provider / 领域请求。
    pub fn display_name(&self, id: ExtensionOperatorId) -> Option<&str> {
        self.names.get(id.0 as usize).map(String::as_str)
    }

    /// 已注册扩展数量。
    pub fn len(&self) -> usize {
        self.names.len()
    }

    /// registry 是否为空。
    pub fn is_empty(&self) -> bool {
        self.names.is_empty()
    }
}
