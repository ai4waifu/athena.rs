//! AthenaIR 方程 → [`Constraint`] 归一化（保留关系方向与 span）。

use athena_ir::{TermNode, TermStore};
use athena_types::{Diagnostic, DiagnosticCode, OperatorId, SourceSpan, TermId};

use super::constraint::{Constraint, ConstraintSet, Equation, Inequality, InequalityOp};

/// 已知比较算子注册表（方言 lowering 注入；禁止字符串匹配）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RelationalOperators {
    /// `Equal`。
    pub equal: OperatorId,
    /// `Less`。
    pub less: OperatorId,
    /// `LessEqual`。
    pub less_equal: OperatorId,
    /// `Greater`。
    pub greater: OperatorId,
    /// `GreaterEqual`。
    pub greater_equal: OperatorId,
    /// `Unequal` / `≠`。
    pub not_equal: OperatorId,
}

impl RelationalOperators {
    /// 测试用占位 id（`0..5`）。调用方须自行保证与构造的 `OperatorId` 一致。
    /// 不依赖任何预置表面算子目录。
    pub fn placeholder() -> Self {
        Self {
            equal: OperatorId(0),
            less: OperatorId(1),
            less_equal: OperatorId(2),
            greater: OperatorId(3),
            greater_equal: OperatorId(4),
            not_equal: OperatorId(5),
        }
    }
}

/// 将二元关系 App 归一化为 [`Constraint`]，**不**压成 `lhs - rhs = 0`。
pub fn normalize_relational_application(
    arena: &TermStore,
    root: TermId,
    ops: &RelationalOperators,
) -> Result<Constraint, Diagnostic> {
    let (kind, span) = arena_node(arena, root)?;
    let TermNode::Application { head: op, arguments: args } = kind
    else {
        return Err(diag("expected_app"));
    };
    if args.len() != 2 {
        return Err(diag("arity_not_binary"));
    }
    let lhs = args[0];
    let rhs = args[1];
    let span = Some(span);
    if *op == ops.equal {
        return Ok(Constraint::Equation(Equation { lhs, rhs, span }));
    }
    let ineq_op = if *op == ops.less {
        InequalityOp::Less
    }
    else if *op == ops.less_equal {
        InequalityOp::LessEqual
    }
    else if *op == ops.greater {
        InequalityOp::Greater
    }
    else if *op == ops.greater_equal {
        InequalityOp::GreaterEqual
    }
    else if *op == ops.not_equal {
        InequalityOp::NotEqual
    }
    else {
        return Err(diag("unknown_relational_operator"));
    };
    Ok(Constraint::Inequality(Inequality { lhs, op: ineq_op, rhs, span }))
}

/// 将若干根项归一化为合取 [`ConstraintSet`]。
pub fn normalize_constraint_conjunction(
    arena: &TermStore,
    roots: &[TermId],
    ops: &RelationalOperators,
) -> Result<ConstraintSet, Diagnostic> {
    let mut members = Vec::with_capacity(roots.len());
    for root in roots {
        members.push(normalize_relational_application(arena, *root, ops)?);
    }
    Ok(ConstraintSet::and(members))
}

fn arena_node(arena: &TermStore, id: TermId) -> Result<(&TermNode, SourceSpan), Diagnostic> {
    let kind = arena.get(id).ok_or_else(|| diag("missing_term"))?;
    let span = arena.span(id).unwrap_or_default();
    Ok((kind, span))
}

fn diag(reason: &str) -> Diagnostic {
    Diagnostic::new(DiagnosticCode::TypeMismatch)
        .detail("domain", "solve")
        .detail("operation", "normalize_relational")
        .detail("reason", reason)
}
