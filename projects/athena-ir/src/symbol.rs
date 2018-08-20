//! Symbol intern table for IR.

use athena_types::SymbolId;

/// Interned symbol names.
#[derive(Debug, Default)]
pub struct SymbolTable {
    names: Vec<String>,
    index: std::collections::HashMap<String, SymbolId>,
}

impl SymbolTable {
    /// Empty table.
    pub fn new() -> Self {
        Self::default()
    }

    /// Intern a symbol name → stable id.
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

    /// Resolve id to name.
    pub fn resolve(&self, id: SymbolId) -> Option<&str> {
        self.names.get(id.0 as usize).map(String::as_str)
    }

    /// Number of interned symbols.
    pub fn len(&self) -> usize {
        self.names.len()
    }

    /// Whether empty.
    pub fn is_empty(&self) -> bool {
        self.names.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn intern_stable() {
        let mut t = SymbolTable::new();
        let a = t.intern("x");
        let b = t.intern("x");
        assert_eq!(a, b);
        assert_eq!(t.resolve(a), Some("x"));
    }
}
