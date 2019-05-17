//! 域分派 — `D` / `Integrate` / `Limit` / `Series` / `DSolve` / `LaplaceTransform` / `Solve`。
//!
//! 微积分与 `Solve` 均经 session arena + [`TermId`]（Living `25`）。

use athena_ir::{AtomKind, TermKind};
use athena_numeric::{Number, add as num_add, mul as num_mul};
use athena_types::TermId;

use crate::numeric_clone::clone_number;

use super::{Outcome, vm::Vm};

pub(crate) fn h_calc_d(vm: &mut Vm<'_>, operands: &[TermId]) -> Outcome {
    calculus_bridge(vm, "D", operands[0])
}

pub(crate) fn h_calc_integrate(vm: &mut Vm<'_>, operands: &[TermId]) -> Outcome {
    calculus_bridge(vm, "Integrate", operands[0])
}

pub(crate) fn h_calc_limit(vm: &mut Vm<'_>, operands: &[TermId]) -> Outcome {
    calculus_bridge(vm, "Limit", operands[0])
}

pub(crate) fn h_calc_series(vm: &mut Vm<'_>, operands: &[TermId]) -> Outcome {
    calculus_bridge(vm, "Series", operands[0])
}

pub(crate) fn h_calc_dsolve(vm: &mut Vm<'_>, operands: &[TermId]) -> Outcome {
    calculus_bridge(vm, "DSolve", operands[0])
}

pub(crate) fn h_calc_laplace(vm: &mut Vm<'_>, operands: &[TermId]) -> Outcome {
    calculus_bridge(vm, "LaplaceTransform", operands[0])
}

fn calculus_bridge(vm: &mut Vm<'_>, name: &str, root: TermId) -> Outcome {
    let args = vm.app_args(root).unwrap_or_default();
    let mut evaluated = Vec::with_capacity(args.len());
    let mut diags = Vec::new();
    for a in args {
        let o = vm.eval_value(a);
        diags.extend(o.diagnostics);
        evaluated.push(o.term);
    }
    if diags.iter().any(|d| d.severity == athena_types::Severity::Error) {
        let echo = vm.push_app(name, evaluated);
        return Outcome {
            term: echo,
            kind: super::EvalKind::Unevaluated,
            status: athena_types::ComputationStatus::Invalid,
            diagnostics: diags,
        };
    }
    let echo = vm.push_app(name, evaluated);
    let req = {
        let mut cc = crate::calculus::ctx::CalculusCtx::new(vm.session);
        crate::calculus::try_calculus_request(&mut cc, echo)
    };
    if let Some(req) = req {
        let result = crate::calculus::execute_calculus(vm.session, req);
        let term = {
            let mut cc = crate::calculus::ctx::CalculusCtx::new(vm.session);
            crate::calculus::calculus_result_bridge_term(&mut cc, &result)
        };
        return Outcome::value(term).with_diagnostics(diags);
    }
    Outcome::unevaluated(echo).with_diagnostics(diags)
}

/// 单变量多项式 `Solve` → `{{x->r},…}`（typed `SolutionSet` 仍为正式合同）。
pub(crate) fn solve(vm: &mut Vm<'_>, equation: TermId, unknown: TermId) -> Outcome {
    let echo = vm.push_app("Solve", vec![equation, unknown]);
    let Some(var_name) = (match vm.session.arena.get(unknown) {
        Some(TermKind::Atom(AtomKind::Symbol(s))) => vm.session.arena.symbols().resolve(*s).map(str::to_string),
        _ => None,
    })
    else {
        return Outcome::unevaluated(echo);
    };

    let zero_id = if vm.head_name(equation).as_deref() == Some("Equal") {
        let args = vm.app_args(equation).unwrap_or_default();
        if args.len() == 2 {
            let neg1 = vm.push_int(-1);
            let neg = vm.push_app("Times", vec![neg1, args[1]]);
            let plus = vm.push_app("Plus", vec![args[0], neg]);
            vm.eval_value(plus).term
        }
        else {
            vm.eval_value(equation).term
        }
    }
    else {
        vm.eval_value(equation).term
    };

    let Some(terms) = collect_univariate_monomials(vm, zero_id, &var_name)
    else {
        return Outcome::unevaluated(echo);
    };
    if terms.is_empty() {
        return Outcome::unevaluated(echo);
    }

    use crate::{
        polynomial::{CoefficientDomain, MonomialOrder, PolynomialBuilder, PolynomialFactorLimits, RingTable},
        solve::{BoundSymbol, CoverageStatus, SolveDomain, solve_univariate_polynomial_roots},
    };
    use athena_types::SymbolId;

    let mut rings = RingTable::new();
    let Ok(ring) = rings.intern(CoefficientDomain::Rational, vec![SymbolId(0)], MonomialOrder::Lex)
    else {
        return Outcome::unevaluated(echo);
    };
    let mut builder = PolynomialBuilder::new(ring);
    for (coeff, deg) in terms {
        if builder.push_term(coeff, vec![deg]).is_err() {
            return Outcome::unevaluated(echo);
        }
    }
    let Ok(poly) = builder.build(&rings)
    else {
        return Outcome::unevaluated(echo);
    };
    let unknown_sym = BoundSymbol::free(SymbolId(0));
    let Ok(adapted) =
        solve_univariate_polynomial_roots(poly, &rings, unknown_sym, SolveDomain::Rationals, PolynomialFactorLimits::default())
    else {
        return Outcome::unevaluated(echo);
    };
    if !matches!(adapted.solution.coverage, CoverageStatus::Complete) {
        return Outcome::unevaluated(echo);
    }

    let mut roots: Vec<TermId> = Vec::new();
    for branch in &adapted.solution.branches {
        let Some(tid) = branch.bindings.get(&unknown_sym)
        else {
            return Outcome::unevaluated(echo);
        };
        let Some(val) = adapted.values.get(tid)
        else {
            return Outcome::unevaluated(echo);
        };
        let root_term = match val {
            crate::solve::BindingValue::Number(n) => push_number_for_solve(vm, n),
            crate::solve::BindingValue::Rational(r) => super::lin::rational_to_term(vm, r),
            crate::solve::BindingValue::MachineF64(_) => return Outcome::unevaluated(echo),
        };
        roots.push(root_term);
    }
    roots.sort_by(|a, b| {
        match (super::arith::num_compare_ids(vm, *a, *b), super::arith::number_of(vm, *a), super::arith::number_of(vm, *b)) {
            (Some(ord), _, _) => ord,
            _ => std::cmp::Ordering::Equal,
        }
    });

    let var_id = vm.push_symbol(&var_name);
    let rule_op = vm.session.operators.intern("Rule");
    let mut out = Vec::with_capacity(roots.len());
    for r in roots {
        let rule = vm.rebuild_app_op(rule_op, vec![var_id, r]);
        out.push(vm.push_list(vec![rule]));
    }
    Outcome::value(vm.push_list(out))
}

fn push_number_for_solve(vm: &mut Vm<'_>, n: &Number) -> TermId {
    if let Some(i) = n.as_exact_integer() {
        return vm.push_int(i);
    }
    if let Some(i) = n.as_integer() {
        if let Some(v) = i.to_i64() {
            return vm.push_int(v);
        }
    }
    if let Some(r) = n.as_rational() {
        return super::lin::rational_to_term(vm, r);
    }
    super::arith::push_number(vm, clone_number(n))
}

/// 将 `Plus`/`Times`/`Power` 展开为单变量 `(coeff, degree)` 项（仅有理系数 · arena 版）。
fn collect_univariate_monomials(vm: &Vm<'_>, expr: TermId, var: &str) -> Option<Vec<(Number, u32)>> {
    fn merge(dst: &mut Vec<(Number, u32)>, src: Vec<(Number, u32)>) -> Option<()> {
        for (c, d) in src {
            if let Some((existing, _)) = dst.iter_mut().find(|(_, ed)| *ed == d) {
                *existing = num_add(clone_number(existing), c).ok()?;
            }
            else {
                dst.push((c, d));
            }
        }
        dst.retain(|(c, _)| !c.is_zero());
        Some(())
    }

    fn mul_lists(a: &[(Number, u32)], b: &[(Number, u32)]) -> Option<Vec<(Number, u32)>> {
        let mut out = Vec::new();
        for (ca, da) in a {
            for (cb, db) in b {
                let c = num_mul(clone_number(ca), clone_number(cb)).ok()?;
                let d = da.checked_add(*db)?;
                merge(&mut out, vec![(c, d)])?;
            }
        }
        Some(out)
    }

    fn is_sym(vm: &Vm<'_>, id: TermId, var: &str) -> bool {
        matches!(
            vm.session.arena.get(id),
            Some(TermKind::Atom(AtomKind::Symbol(s))) if vm.session.arena.symbols().resolve(*s) == Some(var)
        )
    }

    fn go(vm: &Vm<'_>, expr: TermId, var: &str) -> Option<Vec<(Number, u32)>> {
        if is_sym(vm, expr, var) {
            return Some(vec![(Number::small_int(1), 1)]);
        }
        if let Some(n) = super::arith::number_of(vm, expr) {
            return Some(vec![(clone_number(n), 0)]);
        }
        match vm.head_name(expr).as_deref() {
            Some("Plus") => {
                let mut out = Vec::new();
                for a in vm.app_args(expr).unwrap_or_default() {
                    merge(&mut out, go(vm, a, var)?)?;
                }
                Some(out)
            }
            Some("Times") => {
                let mut out = vec![(Number::small_int(1), 0)];
                for a in vm.app_args(expr).unwrap_or_default() {
                    out = mul_lists(&out, &go(vm, a, var)?)?;
                }
                Some(out)
            }
            Some("Power") => {
                let args = vm.app_args(expr).unwrap_or_default();
                if args.len() != 2 {
                    return None;
                }
                let exp = super::arith::number_of(vm, args[1])?.as_integer_exp()?;
                if exp < 0 {
                    return None;
                }
                let exp = exp as u32;
                if is_sym(vm, args[0], var) {
                    return Some(vec![(Number::small_int(1), exp)]);
                }
                let base = go(vm, args[0], var)?;
                if base.len() == 1 && base[0].1 == 0 {
                    let mut p = Number::small_int(1);
                    for _ in 0..exp {
                        p = num_mul(p, clone_number(&base[0].0)).ok()?;
                    }
                    return Some(vec![(p, 0)]);
                }
                // (poly)^n：仅支持已展开低次，保守拒绝。
                if exp == 0 {
                    return Some(vec![(Number::small_int(1), 0)]);
                }
                if exp == 1 {
                    return Some(base);
                }
                None
            }
            Some("List") => None,
            _ => None,
        }
    }

    go(vm, expr, var)
}
