//! 会话 arena 上的符号求导（· typed `DomainExecutionContext`）。

use athena_ir::{ApplicationHead, SemanticOperator};
use athena_types::{CollectionKind, Predicate, Result, SymbolId, TermId};

use crate::{domains::context::DomainExecutionContext, execution::builtins::registry::lookup_unary};

use super::result::{ConditionalResult, unresolved};

/// 在 arena 上做符号求导。执行失败经 [`Result`] 传播，禁止吞错。
pub fn differentiate(dc: &mut DomainExecutionContext<'_>, expr: TermId, var: SymbolId) -> Result<TermId> {
    differentiate_symbol(dc, expr, var)
}

fn differentiate_symbol(dc: &mut DomainExecutionContext<'_>, expr: TermId, var: SymbolId) -> Result<TermId> {
    let Some(shape) = dc.shape(expr)
    else {
        return Ok(expr);
    };
    Ok(match shape {
        crate::execution::shape::Shape::Number
        | crate::execution::shape::Shape::String(_)
        | crate::execution::shape::Shape::Bool(_)
        | crate::execution::shape::Shape::Null
        | crate::execution::shape::Shape::Constant(_) => dc.in_(0),
        crate::execution::shape::Shape::Symbol(s) => dc.in_(if dc.symbol_id_is(s, var) { 1 } else { 0 }),
        crate::execution::shape::Shape::Collection(items) => {
            let mut ds = Vec::with_capacity(items.len());
            for i in items {
                ds.push(differentiate_symbol(dc, i, var)?);
            }
            dc.collection(CollectionKind::OrderedCollection, ds)
        }
        crate::execution::shape::Shape::Application(head, args) => match head {
            ApplicationHead::Semantic(SemanticOperator::Add) => {
                let mut ds = Vec::with_capacity(args.len());
                for a in &args {
                    ds.push(differentiate_symbol(dc, *a, var)?);
                }
                dc.fold_term(dc.apply_semantic(SemanticOperator::Add, ds))?
            }
            ApplicationHead::Semantic(SemanticOperator::Multiply) => {
                let mut terms = Vec::new();
                for i in 0..args.len() {
                    let mut factors = args.clone();
                    factors[i] = differentiate_symbol(dc, args[i], var)?;
                    terms.push(dc.apply_semantic(SemanticOperator::Multiply, factors));
                }
                dc.fold_term(dc.apply_semantic(SemanticOperator::Add, terms))?
            }
            ApplicationHead::Semantic(SemanticOperator::Power) if args.len() == 2 => {
                let base = args[0];
                let exp = args[1];
                if let Some(n) = dc.int_exp(exp) {
                    let n1 = dc.in_(n - 1);
                    let pow = dc.apply_semantic(SemanticOperator::Power, vec![base, n1]);
                    let d = differentiate_symbol(dc, base, var)?;
                    dc.fold_term(dc.apply_semantic(SemanticOperator::Multiply, vec![dc.in_(n), pow, d]))?
                } else if let Some(nf) = dc.number_of(exp).and_then(|n| n.as_machine_f64()) {
                    let base_pow = dc.apply_semantic(SemanticOperator::Power, vec![base, dc.real(nf - 1.0)]);
                    let d = differentiate_symbol(dc, base, var)?;
                    dc.fold_term(dc.apply_semantic(SemanticOperator::Multiply, vec![dc.real(nf), base_pow, d]))?
                } else {
                    residual_diff(dc, expr, var)
                }
            }
            ApplicationHead::Semantic(SemanticOperator::Subtract) if args.len() == 2 => {
                let d0 = differentiate_symbol(dc, args[0], var)?;
                let d1 = differentiate_symbol(dc, args[1], var)?;
                let neg = dc.apply_semantic(SemanticOperator::Multiply, vec![dc.in_(-1), d1]);
                dc.fold_term(dc.apply_semantic(SemanticOperator::Add, vec![d0, neg]))?
            }
            ApplicationHead::Semantic(SemanticOperator::Divide) if args.len() == 2 => {
                let (a, b) = (args[0], args[1]);
                let da = differentiate_symbol(dc, a, var)?;
                let db = differentiate_symbol(dc, b, var)?;
                let t1 = dc.apply_semantic(SemanticOperator::Multiply, vec![da, b]);
                let t2 = dc.apply_semantic(SemanticOperator::Multiply, vec![dc.in_(-1), a, db]);
                let plus = dc.apply_semantic(SemanticOperator::Add, vec![t1, t2]);
                let binv = dc.apply_semantic(SemanticOperator::Power, vec![b, dc.in_(-2)]);
                dc.fold_term(dc.apply_semantic(SemanticOperator::Multiply, vec![plus, binv]))?
            }
            ApplicationHead::Semantic(SemanticOperator::Abs | SemanticOperator::Sqrt) if args.len() == 1 => residual_diff(dc, expr, var),
            ApplicationHead::Semantic(op) => {
                if let Some(uf) = op.as_unary() {
                    if let Some(def) = lookup_unary(uf) {
                        if def.arity == 1 && args.len() == 1 {
                            if let Some(df) = def.unary_derivative {
                                let outer = df(dc, args[0]);
                                let inner = differentiate_symbol(dc, args[0], var)?;
                                return Ok(dc.fold_term(dc.apply_semantic(SemanticOperator::Multiply, vec![outer, inner]))?);
                            }
                        }
                    }
                }
                residual_diff(dc, expr, var)
            }
            ApplicationHead::Extension(_) => residual_diff(dc, expr, var),
        },
    })
}

fn residual_diff(dc: &mut DomainExecutionContext<'_>, expr: TermId, var: SymbolId) -> TermId {
    dc.apply_semantic(SemanticOperator::Differentiate, vec![expr, dc.symbol_id(var)])
}

/// 在假设下求导。执行失败经 [`Result`] 传播；数学条件仍在 [`ConditionalResult`]。
pub fn differentiate_checked(
    dc: &mut DomainExecutionContext<'_>,
    expr: TermId,
    var: SymbolId,
    assumptions: &athena_types::AssumptionSet,
) -> Result<ConditionalResult<TermId>> {
    if let Some((head, args)) = dc.application_head(expr) {
        if matches!(head, ApplicationHead::Semantic(SemanticOperator::Abs)) && args.len() == 1 {
            let inner = args[0];
            let abs = dc.apply_semantic(SemanticOperator::Abs, vec![inner]);
            let binv = dc.apply_semantic(SemanticOperator::Power, vec![inner, dc.in_(-1)]);
            let d = differentiate_symbol(dc, inner, var)?;
            let candidate = dc.fold_term(dc.apply_semantic(SemanticOperator::Multiply, vec![abs, binv, d]))?;
            let needs_nonzero = !assumptions.predicates.iter().any(|p| matches!(p, Predicate::NonZero(_) | Predicate::SymbolNonZero(_)));
            if needs_nonzero {
                return Ok(ConditionalResult::with_unresolved(candidate, vec![unresolved(Predicate::NonZero(athena_types::TermId(0)))]));
            }
            return Ok(ConditionalResult::exact(candidate));
        }
        if matches!(head, ApplicationHead::Semantic(SemanticOperator::Sqrt)) && args.len() == 1 {
            let inner = args[0];
            let sqrt = dc.apply_semantic(SemanticOperator::Sqrt, vec![inner]);
            let two_sqrt = dc.apply_semantic(SemanticOperator::Multiply, vec![dc.in_(2), sqrt]);
            let binv = dc.apply_semantic(SemanticOperator::Power, vec![two_sqrt, dc.in_(-1)]);
            let d = differentiate_symbol(dc, inner, var)?;
            let candidate = dc.fold_term(dc.apply_semantic(SemanticOperator::Multiply, vec![binv, d]))?;
            let needs_nonneg = !assumptions.predicates.iter().any(|p| matches!(p, Predicate::NonNegative(_) | Predicate::Positive(_)));
            if needs_nonneg {
                return Ok(ConditionalResult::with_unresolved(candidate, vec![unresolved(Predicate::NonNegative(athena_types::TermId(0)))]));
            }
            return Ok(ConditionalResult::exact(candidate));
        }
    }
    let d = differentiate_symbol(dc, expr, var)?;
    Ok(ConditionalResult::exact(dc.fold_term(d)?))
}
