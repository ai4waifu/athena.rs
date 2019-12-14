//! `TermStore` — Core IR 唯一符号项存储。

use std::collections::HashMap;

use athena_types::{CollectionKind, Diagnostic, DiagnosticCode, Result, SourceSpan, SymbolId, TermId};

use crate::{
    node::{Atom, MathematicalConstant, TermNode},
    operator::ApplicationHead,
    symbol::SymbolTable,
};

/// Structural key for hash-consing (Living `26` · stable identity, not arena index).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
enum ConsKey {
    Symbol(SymbolId),
    Boolean(bool),
    Null,
    Constant(MathematicalConstant),
    String(String),
    /// Number identity via render + domain tag (avoids requiring `Hash` on `NumericValue`).
    Number {
        render: String,
        domain_tag: u64,
    },
    Collection {
        kind: CollectionKind,
        elements: Vec<TermId>,
    },
    Application {
        head: ApplicationHead,
        arguments: Vec<TermId>,
    },
}

fn cons_key(kind: &TermNode) -> ConsKey {
    match kind {
        TermNode::Atom(Atom::Symbol(id)) => ConsKey::Symbol(*id),
        TermNode::Atom(Atom::Boolean(b)) => ConsKey::Boolean(*b),
        TermNode::Atom(Atom::Null) => ConsKey::Null,
        TermNode::Atom(Atom::Constant(c)) => ConsKey::Constant(*c),
        TermNode::Atom(Atom::String(s)) => ConsKey::String(s.clone()),
        TermNode::Atom(Atom::Number(n)) => ConsKey::Number {
            render: n.to_render_string(),
            domain_tag: n.fingerprint_domain_tag(),
        },
        TermNode::Collection { kind, elements } => ConsKey::Collection {
            kind: *kind,
            elements: elements.clone(),
        },
        TermNode::Application { head, arguments } => ConsKey::Application {
            head: *head,
            arguments: arguments.clone(),
        },
    }
}

/// Core CAS IR 符号项存储。
#[derive(Debug, Default)]
pub struct TermStore {
    nodes: Vec<TermNode>,
    spans: Vec<SourceSpan>,
    symbols: SymbolTable,
    cons: HashMap<ConsKey, TermId>,
}

impl TermStore {
    /// 空存储。
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

    /// 分配或复用 term 节点（hash-cons），返回稳定 [`TermId`]。
    ///
    /// Equal structure under [`ConsKey`] shares one id. First-seen span is kept.
    pub fn push(&mut self, kind: TermNode, span: SourceSpan) -> TermId {
        let key = cons_key(&kind);
        if let Some(id) = self.cons.get(&key) {
            return *id;
        }
        let id = TermId(self.nodes.len() as u32);
        self.nodes.push(kind);
        self.spans.push(span);
        self.cons.insert(key, id);
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
        (
            Some(TermNode::Collection { kind: kx, elements: xs }),
            Some(TermNode::Collection { kind: ky, elements: ys }),
        ) => kx == ky && xs.len() == ys.len() && xs.iter().zip(ys.iter()).all(|(a, b)| structural_eq_walk(arena, *a, *b, seen)),
        (Some(TermNode::Application { head: op_x, arguments: xs }), Some(TermNode::Application { head: op_y, arguments: ys })) => {
            op_x == op_y && xs.len() == ys.len() && xs.iter().zip(ys.iter()).all(|(a, b)| structural_eq_walk(arena, *a, *b, seen))
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
    let result = match kind {
        TermNode::Atom(_) => Ok(()),
        TermNode::Collection { elements, .. } => {
            for child in elements {
                verify_term(arena, *child, stack)?;
            }
            Ok(())
        }
        TermNode::Application { arguments, .. } => {
            for child in arguments {
                verify_term(arena, *child, stack)?;
            }
            Ok(())
        }
    };
    stack.pop();
    result
}

#[cfg(test)]
mod tests {
    use athena_types::SourceSpan;

    use super::*;
    use crate::{ApplicationHead, SemanticOperator};

    #[test]
    fn push_hash_conses_equal_atoms_and_applications() {
        let mut store = TermStore::new();
        let span = SourceSpan::default();
        let a = store.push(TermNode::Atom(Atom::Boolean(true)), span);
        let b = store.push(TermNode::Atom(Atom::Boolean(true)), span);
        assert_eq!(a, b);
        assert_eq!(store.len(), 1);

        let one = store.push(
            TermNode::Atom(Atom::Number(athena_numeric::Number::small_int(1))),
            span,
        );
        let one_again = store.push(
            TermNode::Atom(Atom::Number(athena_numeric::Number::small_int(1))),
            span,
        );
        assert_eq!(one, one_again);

        let add1 = store.push(
            TermNode::Application {
                head: ApplicationHead::Semantic(SemanticOperator::Add),
                arguments: vec![one, one],
            },
            span,
        );
        let add2 = store.push(
            TermNode::Application {
                head: ApplicationHead::Semantic(SemanticOperator::Add),
                arguments: vec![one, one],
            },
            span,
        );
        assert_eq!(add1, add2);
        assert_eq!(store.len(), 3);
    }
}
