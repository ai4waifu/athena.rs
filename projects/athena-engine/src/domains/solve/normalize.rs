//! AthenaIR 方程 → [`Constraint`] 归一化（保留关系方向与 span）。

use athena_ir::{ApplicationHead, SemanticOperator, TermNode, TermStore};
use athena_types::{Diagnostic, DiagnosticCode, SourceSpan, TermId};

use super::constraint::{Constraint, ConstraintSet, Equation, Inequality, InequalityOp};

/// 将二元关系 App 归一化为 [`Constraint`]，**不**压成 `lhs - rhs = 0`。
pub fn normalize_relational_application(arena: &TermStore, root: TermId) -> Result<Constraint, Diagnostic> {
    let (kind, span) = arena_node(arena, root)?;
    let TermNode::Application { head, arguments: args } = kind
    else {
        return Err(diag("expected_app"));
    };
    if args.len() != 2 {
        return Err(diag("arity_not_binary"));
    }
    let lhs = args[0];
    let rhs = args[1];
    let span = Some(span);
    let ApplicationHead::Semantic(op) = *head
    else {
        return Err(diag("unknown_relational_operator"));
    };
    match op {
        SemanticOperator::Equal => Ok(Constraint::Equation(Equation { lhs, rhs, span })),
        SemanticOperator::Less => Ok(Constraint::Inequality(Inequality { lhs, op: InequalityOp::Less, rhs, span })),
        SemanticOperator::LessEqual => Ok(Constraint::Inequality(Inequality { lhs, op: InequalityOp::LessEqual, rhs, span })),
        SemanticOperator::Greater => Ok(Constraint::Inequality(Inequality { lhs, op: InequalityOp::Greater, rhs, span })),
        SemanticOperator::GreaterEqual => Ok(Constraint::Inequality(Inequality { lhs, op: InequalityOp::GreaterEqual, rhs, span })),
        SemanticOperator::Unequal => Ok(Constraint::Inequality(Inequality { lhs, op: InequalityOp::NotEqual, rhs, span })),
        _ => Err(diag("unknown_relational_operator")),
    }
}

/// 将若干根项归一化为合取 [`ConstraintSet`]。
pub fn normalize_constraint_conjunction(arena: &TermStore, roots: &[TermId]) -> Result<ConstraintSet, Diagnostic> {
    let mut members = Vec::with_capacity(roots.len());
    for root in roots {
        members.push(normalize_relational_application(arena, *root)?);
    }
    Ok(ConstraintSet::and(members))
}

fn arena_node(arena: &TermStore, id: TermId) -> Result<(&TermNode, SourceSpan), Diagnostic> {
    let kind = arena.get(id).ok_or_else(|| diag("missing_term"))?;
    let span = arena.span(id).unwrap_or_default();
    Ok((kind, span))
}

fn diag(reason: &str) -> Diagnostic {
    Diagnostic::new(DiagnosticCode::UnsupportedOperation).detail("component", "solve_normalize").detail("reason", reason)
}
