//! Term arena — Core IR 唯一存储。

use athena_types::{Diagnostic, DiagnosticCode, Result, SourceSpan, TermId};

use crate::{node::TermNode, symbol::SymbolTable};

/// 基于 arena 的 Core CAS IR。
#[derive(Debug, Default)]
pub struct TermStore {
    nodes: Vec<TermNode>,
    spans: Vec<SourceSpan>,
    symbols: SymbolTable,
}

impl TermStore {
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
    pub fn push(&mut self, kind: TermNode, span: SourceSpan) -> TermId {
        let id = TermId(self.nodes.len() as u32);
        self.nodes.push(kind);
        self.spans.push(span);
        id
    }

    /// 按 id 取 term 节点。
    pub fn get(&self, id: TermId) -> Option<&TermNode> {
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

    /// 结构等价（数值载荷按 [`NumericValue`](athena_numeric::NumericValue) 精确相等）。
    ///
    /// DAG 共享子图去重；与插入地址无关，只比结构与载荷。
    pub fn structural_eq(&self, a: TermId, b: TermId) -> bool {
        let mut seen = std::collections::HashSet::new();
        structural_eq_walk(self, a, b, &mut seen)
    }
}

fn structural_eq_walk(arena: &TermStore, x: TermId, y: TermId, seen: &mut std::collections::HashSet<(u32, u32)>) -> bool {
    if x == y {
        return true;
    }
    if !seen.insert((x.0, y.0)) {
        return true;
    }
    match (arena.get(x), arena.get(y)) {
        (Some(TermNode::Atom(p)), Some(TermNode::Atom(q))) => p == q,
        (Some(TermNode::List(xs)), Some(TermNode::List(ys))) => {
            xs.len() == ys.len() && xs.iter().zip(ys.iter()).all(|(a, b)| structural_eq_walk(arena, *a, *b, seen))
        }
        (Some(TermNode::Application { head: op_x, arguments: xs }), Some(TermNode::Application { head: op_y, arguments: ys })) => {
            op_x == op_y
                && xs.len() == ys.len()
                && xs.iter().zip(ys.iter()).all(|(a, b)| structural_eq_walk(arena, *a, *b, seen))
        }
        _ => false,
    }
}

fn verify_term(arena: &TermStore, id: TermId, stack: &mut Vec<TermId>) -> Result<()> {
    if stack.contains(&id) {
        return Err(Diagnostic::new(DiagnosticCode::UnsupportedOperation));
    }
    let Some(kind) = arena.get(id)
    else {
        return Err(Diagnostic::new(DiagnosticCode::InvalidIndex));
    };
    stack.push(id);
    match kind {
        TermNode::Atom(_) => {}
        TermNode::List(items) | TermNode::Application { arguments: items, .. } => {
            for child in items {
                verify_term(arena, *child, stack)?;
            }
        }
    }
    stack.pop();
    Ok(())
}
