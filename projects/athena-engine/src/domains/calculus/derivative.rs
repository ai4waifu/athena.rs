//! 会话 arena 上的符号求导（Living `25` L3 · `TermId` 进出）。

use athena_ir::SemanticOperator;
use athena_types::{Predicate, TermId};

use crate::execution::builtins::registry::lookup_function;

use super::{
    ctx::CalculusCtx,
    result::{ConditionalResult, unresolved},
};

/// 在 arena 上做符号求导。
pub fn differentiate(cc: &mut CalculusCtx<'_>, expr: TermId, var: &str) -> TermId {
    let Some(shape) = cc.shape(expr)
    else {
        return expr;
    };
    match shape {
        crate::execution::shape::Shape::Number
        | crate::execution::shape::Shape::String(_)
        | crate::execution::shape::Shape::Bool(_)
        | crate::execution::shape::Shape::Null => cc.in_(0),
        crate::execution::shape::Shape::Symbol(s) => cc.in_(if cc.symbol_is(s, var) { 1 } else { 0 }),
        crate::execution::shape::Shape::Collection(items) => {
            let ds = items.iter().map(|i| differentiate(cc, *i, var)).collect();
            cc.list(ds)
        }
        crate::execution::shape::Shape::Application(_, args) => {
            let Some((h, args)) = cc.application(expr)
            else {
                return expr;
            };
            match h.as_str() {
                "Plus" => {
                    let ds = args.iter().map(|a| differentiate(cc, *a, var)).collect();
                    cc.eval(cc.apply_semantic(SemanticOperator::Add, ds))
                }
                "Times" => {
                    let mut terms = Vec::new();
                    for i in 0..args.len() {
                        let mut factors = args.clone();
                        factors[i] = differentiate(cc, args[i], var);
                        terms.push(cc.apply_semantic(SemanticOperator::Multiply, factors));
                    }
                    cc.eval(cc.apply_semantic(SemanticOperator::Add, terms))
                }
                "Power" if args.len() == 2 => {
                    let base = args[0];
                    let exp = args[1];
                    if let Some(n) = cc.int_exp(exp) {
                        let n1 = cc.in_(n - 1);
                        let pow = cc.apply_semantic(SemanticOperator::Power, vec![base, n1]);
                        let d = differentiate(cc, base, var);
                        cc.eval(cc.apply_semantic(SemanticOperator::Multiply, vec![cc.in_(n), pow, d]))
                    }
                    else if let Some(nf) = cc.number_of(exp).map(|n| cc.copy(n)).and_then(|n| n.as_machine_f64()) {
                        let base_pow = cc.apply_semantic(SemanticOperator::Power, vec![base, cc.real(nf - 1.0)]);
                        let d = differentiate(cc, base, var);
                        cc.eval(cc.apply_semantic(SemanticOperator::Multiply, vec![cc.real(nf), base_pow, d]))
                    }
                    else {
                        cc.apply("D", vec![expr, cc.symbol(var)])
                    }
                }
                "Subtract" if args.len() == 2 => {
                    let d0 = differentiate(cc, args[0], var);
                    let d1 = differentiate(cc, args[1], var);
                    let neg = cc.apply_semantic(SemanticOperator::Multiply, vec![cc.in_(-1), d1]);
                    cc.eval(cc.apply_semantic(SemanticOperator::Add, vec![d0, neg]))
                }
                "Divide" if args.len() == 2 => {
                    let (a, b) = (args[0], args[1]);
                    let da = differentiate(cc, a, var);
                    let db = differentiate(cc, b, var);
                    let t1 = cc.apply_semantic(SemanticOperator::Multiply, vec![da, b]);
                    let t2 = cc.apply_semantic(SemanticOperator::Multiply, vec![cc.in_(-1), a, db]);
                    let plus = cc.apply_semantic(SemanticOperator::Add, vec![t1, t2]);
                    let binv = cc.apply_semantic(SemanticOperator::Power, vec![b, cc.in_(-2)]);
                    cc.eval(cc.apply_semantic(SemanticOperator::Multiply, vec![plus, binv]))
                }
                // Abs / Sqrt：无条件路径保留 D；条件路径见 [`differentiate_checked`]。
                "Abs" | "Sqrt" if args.len() == 1 => cc.apply("D", vec![expr, cc.symbol(var)]),
                _ => {
                    if let Some(def) = lookup_function(&h) {
                        if def.arity == 1 && args.len() == 1 {
                            if let Some(df) = def.unary_derivative {
                                let outer = df(cc, args[0]);
                                let inner = differentiate(cc, args[0], var);
                                return cc.eval(cc.apply_semantic(SemanticOperator::Multiply, vec![outer, inner]));
                            }
                        }
                    }
                    // 未知头部：保留 D，禁止静默当成 0。
                    cc.apply("D", vec![expr, cc.symbol(var)])
                }
            }
        }
    }
}

/// 在假设下求导，返回条件而非裸项。
pub fn differentiate_checked(
    cc: &mut CalculusCtx<'_>,
    expr: TermId,
    var: &str,
    assumptions: &athena_types::AssumptionSet,
) -> ConditionalResult<TermId> {
    if let Some((h, args)) = cc.application(expr) {
        if h == "Abs" && args.len() == 1 {
            let inner = args[0];
            let abs = cc.apply_semantic(SemanticOperator::Abs, vec![inner]);
            let binv = cc.apply_semantic(SemanticOperator::Power, vec![inner, cc.in_(-1)]);
            let d = differentiate(cc, inner, var);
            let candidate = cc.eval(cc.apply_semantic(SemanticOperator::Multiply, vec![abs, binv, d]));
            let needs_nonzero = !assumptions.predicates.iter().any(|p| matches!(p, Predicate::NonZero(_) | Predicate::SymbolNonZero(_)));
            if needs_nonzero {
                // `TermId(0)` 为桥接占位，直至 Abs 参数绑定落地。
                return ConditionalResult::with_unresolved(candidate, vec![unresolved(Predicate::NonZero(athena_types::TermId(0)))]);
            }
            return ConditionalResult::exact(candidate);
        }
        if h == "Sqrt" && args.len() == 1 {
            let inner = args[0];
            let sqrt = cc.apply_semantic(SemanticOperator::Sqrt, vec![inner]);
            let two_sqrt = cc.apply_semantic(SemanticOperator::Multiply, vec![cc.in_(2), sqrt]);
            let binv = cc.apply_semantic(SemanticOperator::Power, vec![two_sqrt, cc.in_(-1)]);
            let d = differentiate(cc, inner, var);
            let candidate = cc.eval(cc.apply_semantic(SemanticOperator::Multiply, vec![binv, d]));
            let needs_nonneg = !assumptions.predicates.iter().any(|p| matches!(p, Predicate::NonNegative(_) | Predicate::Positive(_)));
            if needs_nonneg {
                return ConditionalResult::with_unresolved(candidate, vec![unresolved(Predicate::NonNegative(athena_types::TermId(0)))]);
            }
            return ConditionalResult::exact(candidate);
        }
    }
    let d = differentiate(cc, expr, var);
    ConditionalResult::exact(cc.eval(d))
}
