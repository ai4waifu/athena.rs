//! Term arena — Core IR 唯一存储。

use athena_types::{Diagnostic, DiagnosticCode, Result, SourceSpan, TermId};

use crate::{node::TermKind, symbol::SymbolTable};

/// Arena -backed Core CAS IR。
#[derive(Debug, Default)]
pub struct TermArena {
    nodes: Vec<TermKind>,
    spans: Vec<SourceSpan>,
    symbols: SymbolTable,
}

impl TermArena {
    /// 空 arena。
    pub fn new() -> Self {
        Self::default()
    }

    /// 符号 intern 表。
    pub fn symbols(&self) -> &SymbolTable {
        &self.symbols
    }

    /// 可变符号表（builder 使用）。
    pub fn symbols_mut(&mut self) -> &mut SymbolTable {
        &mut self.symbols
    }

    /// 分配 term 节点，返回稳定 [`TermId`]。
    pub fn push(&mut self, kind: TermKind, span: SourceSpan) -> TermId {
        let id = TermId(self.nodes.len() as u32);
        self.nodes.push(kind);
        self.spans.push(span);
        id
    }

    /// 按 id 取 term 节点。
    pub fn get(&self, id: TermId) -> Option<&TermKind> {
        self.nodes.get(id.0 as usize)
    }

    /// 按 id 取 span。
    pub fn span(&self, id: TermId) -> Option<SourceSpan> {
        self.spans.get(id.0 as usize).copied()
    }

    /// 节点数量。
    pub fn len(&self) -> usize {
        self.nodes.len()
    }

    /// 是否为空。
    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty()
    }

    /// 结构完整性检查（索引有效、无环）。
    pub fn verify(&self, root: TermId) -> Result<()> {
        verify_term(self, root, &mut vec![])
    }
}

fn verify_term(arena: &TermArena, id: TermId, stack: &mut Vec<TermId>) -> Result<()> {
    if stack.contains(&id) {
        return Err(Diagnostic::new(DiagnosticCode::UnsupportedOperation));
    }
    let Some(kind) = arena.get(id)
    else {
        return Err(Diagnostic::new(DiagnosticCode::InvalidIndex));
    };
    stack.push(id);
    match kind {
        TermKind::Atom(_) => {}
        TermKind::List(items) | TermKind::App { args: items, .. } => {
            for child in items {
                verify_term(arena, *child, stack)?;
            }
        }
    }
    stack.pop();
    Ok(())
}
