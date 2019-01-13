//! IR 符号 intern 表。

use athena_types::SymbolId;

/// 已 intern 的符号名。
#[derive(Debug, Default)]
pub struct SymbolTable {
    names: Vec<String>,
    index: std::collections::HashMap<String, SymbolId>,
}

impl SymbolTable {
    /// 空表。
    pub fn new() -> Self {
        Self::default()
    }

    /// intern 符号名 → 稳定 id。
    pub fn intern(&mut self, name: impl Into<String>) -> SymbolId {
        let name = name.into();
        if let Some(id) = self.index.get(&name) {
            return *id;
        }
        let id = SymbolId(self.names.len() as u32);
        self.names.push(name.clone());
        self.index.insert(name, id);
        id
    }

    /// 将 id 解析为名字。
    pub fn resolve(&self, id: SymbolId) -> Option<&str> {
        self.names.get(id.0 as usize).map(String::as_str)
    }

    /// 已 intern 符号数量。
    pub fn len(&self) -> usize {
        self.names.len()
    }

    /// 是否为空。
    pub fn is_empty(&self) -> bool {
        self.names.is_empty()
    }
}
