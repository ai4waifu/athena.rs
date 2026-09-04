//! Term arena — Core IR 唯一存储。

use athena_types::{Diagnostic, DiagnosticCode, ExprId, Result, SourceSpan};

use crate::{node::ExprNode, symbol::SymbolTable};

/// 基于 arena 的 Core CAS IR。
#[derive(Debug, Default)]
pub struct ExprArena {
    nodes: Vec<ExprNode>,
    spans: Vec<SourceSpan>,
    symbols: SymbolTable,
}

impl ExprArena {
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

    /// 分配 term 节点，返回稳定 [`ExprId`]。
    pub fn push(&mut self, kind: ExprNode, span: SourceSpan) -> ExprId {
        let id = ExprId(self.nodes.len() as u32);
        self.nodes.push(kind);
        self.spans.push(span);
        id
    }

    /// 按 id 取 term 节点。
    pub fn get(&self, id: ExprId) -> Option<&ExprNode> {
        self.nodes.get(id.0 as usize)
    }

    /// 按 id 取 span。
    pub fn span(&self, id: ExprId) -> Option<SourceSpan> {
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
    pub fn verify(&self, root: ExprId) -> Result<()> {
        verify_term(self, root, &mut vec![])
    }

    /// 结构等价（数值载荷按 [`NumericValue`](athena_numeric::NumericValue) 精确相等）。
    ///
    /// DAG 共享子图去重；与插入地址无关，只比结构与载荷。
    pub fn structural_eq(&self, a: ExprId, b: ExprId) -> bool {
        let mut seen = std::collections::HashSet::new();
        structural_eq_walk(self, a, b, &mut seen)
    }
}

fn structural_eq_walk(arena: &ExprArena, x: ExprId, y: ExprId, seen: &mut std::collections::HashSet<(u32, u32)>) -> bool {
    if x == y {
        return true;
    }
    if !seen.insert((x.0, y.0)) {
        return true;
    }
    match (arena.get(x), arena.get(y)) {
        (Some(ExprNode::Atom(p)), Some(ExprNode::Atom(q))) => p == q,
        (Some(ExprNode::List(xs)), Some(ExprNode::List(ys))) => {
            xs.len() == ys.len() && xs.iter().zip(ys.iter()).all(|(a, b)| structural_eq_walk(arena, *a, *b, seen))
        }
        (Some(ExprNode::App { op: op_x, args: xs }), Some(ExprNode::App { op: op_y, args: ys })) => {
            op_x == op_y
                && xs.len() == ys.len()
                && xs.iter().zip(ys.iter()).all(|(a, b)| structural_eq_walk(arena, *a, *b, seen))
        }
        _ => false,
    }
}

fn verify_term(arena: &ExprArena, id: ExprId, stack: &mut Vec<ExprId>) -> Result<()> {
    if stack.contains(&id) {
        return Err(Diagnostic::new(DiagnosticCode::UnsupportedOperation));
    }
    let Some(kind) = arena.get(id)
    else {
        return Err(Diagnostic::new(DiagnosticCode::InvalidIndex));
    };
    stack.push(id);
    match kind {
        ExprNode::Atom(_) => {}
        ExprNode::List(items) | ExprNode::App { args: items, .. } => {
            for child in items {
                verify_term(arena, *child, stack)?;
            }
        }
    }
    stack.pop();
    Ok(())
}
