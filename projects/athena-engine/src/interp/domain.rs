//! 域分派桥 — `D` / `Integrate` / `Limit` / `Series` / `DSolve` / `LaplaceTransform` / `Solve`。
//!
//! 过渡实现：arena id ↔ engine 内部微积分桥的私有转换（Living `25` 阶段 3 之后
//! `CalculusRequest` 载荷 `TermId` 化时整体删除，禁止成为公共兼容 API）。

use athena_ir::{AtomKind, TermKind};
use athena_types::TermId;

use crate::term::{Atom, Term};

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
    let echo = vm.push_app(name, evaluated.clone());
    let legacy_app = app_legacy(vm, name, &evaluated);
    if let Some(req) = crate::calculus::try_calculus_request(&legacy_app) {
        let result = crate::calculus::execute_calculus(req);
        let bridged = crate::calculus::calculus_result_bridge_term(&result);
        let term = from_legacy_term(vm, &bridged);
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
    let legacy_eq = to_legacy_term(vm, equation);
    let zero_expr = match &legacy_eq {
        Term::Application { head, arguments } if head.is_symbol("Equal") && arguments.len() == 2 => {
            crate::eval::evaluate(&Term::apply(
                "Plus",
                vec![clone_legacy(&arguments[0]), Term::apply("Times", vec![Term::int(-1), clone_legacy(&arguments[1])])],
            ))
        }
        other => crate::eval::evaluate(&clone_legacy(other)),
    };
    let Some(terms) = crate::eval::collect_univariate_monomials_for_solve(&zero_expr, &var_name)
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
            crate::solve::BindingValue::Number(n) => Term::number(crate::numeric_clone::clone_number(n)),
            crate::solve::BindingValue::Rational(r) => crate::eval::rational_to_term_for_solve(r),
            crate::solve::BindingValue::MachineF64(_) => return Outcome::unevaluated(echo),
        };
        roots.push(from_legacy_term(vm, &root_term));
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

fn app_legacy(vm: &Vm<'_>, name: &str, args: &[TermId]) -> Term {
    Term::Application { head: Box::new(Term::symbol(name)), arguments: args.iter().map(|a| to_legacy_term(vm, *a)).collect() }
}

fn clone_legacy(t: &Term) -> Term {
    crate::numeric_clone::clone_term(t)
}

/// arena 子树 → legacy 树（过渡私有；`Number` 走 `clone_number`）。
pub(crate) fn to_legacy_term(vm: &Vm<'_>, id: TermId) -> Term {
    let Some(kind) = vm.session.arena.get(id)
    else {
        return Term::null();
    };
    match kind {
        TermKind::Atom(AtomKind::Number(n)) => Term::number(crate::numeric_clone::clone_number(n)),
        TermKind::Atom(AtomKind::String(s)) => Term::Atom(Atom::String(s.clone())),
        TermKind::Atom(AtomKind::Symbol(s)) => Term::symbol(vm.session.arena.symbols().resolve(*s).unwrap_or("?").to_string()),
        TermKind::Atom(AtomKind::Boolean(b)) => Term::boolean(*b),
        TermKind::Atom(AtomKind::Null) => Term::null(),
        TermKind::List(items) => Term::List(items.iter().map(|i| to_legacy_term(vm, *i)).collect()),
        TermKind::App { op, args } => {
            let name = vm.session.operators.name(*op).unwrap_or("?").to_string();
            let arguments: Vec<Term> = args.iter().map(|a| to_legacy_term(vm, *a)).collect();
            if name == "Application" && !arguments.is_empty() {
                let mut it = arguments.into_iter();
                let head = Box::new(it.next().unwrap_or_else(|| Term::null()));
                return Term::Application { head, arguments: it.collect() };
            }
            Term::Application { head: Box::new(Term::symbol(name)), arguments }
        }
    }
}

/// legacy 树 → arena 子树（过渡私有）。
pub(crate) fn from_legacy_term(vm: &mut Vm<'_>, term: &Term) -> TermId {
    match term {
        Term::Atom(Atom::Number(n)) => {
            let span = TermKind::default_span();
            vm.session.arena.push(TermKind::Atom(AtomKind::Number(crate::numeric_clone::clone_number(n))), span)
        }
        Term::Atom(Atom::String(s)) => {
            let span = TermKind::default_span();
            vm.session.arena.push(TermKind::Atom(AtomKind::String(s.clone())), span)
        }
        Term::Atom(Atom::Symbol(s)) => vm.push_symbol(s),
        Term::Atom(Atom::Boolean(b)) => vm.push_bool(*b),
        Term::Atom(Atom::Null) => vm.push_null(),
        Term::List(items) => {
            let ids: Vec<TermId> = items.iter().map(|i| from_legacy_term(vm, i)).collect();
            vm.push_list(ids)
        }
        Term::Application { head, arguments } => {
            if let Term::Atom(Atom::Symbol(s)) = head.as_ref() {
                let args: Vec<TermId> = arguments.iter().map(|a| from_legacy_term(vm, a)).collect();
                vm.push_app(s, args)
            }
            else {
                let head_id = from_legacy_term(vm, head);
                let mut wrapped = vec![head_id];
                wrapped.extend(arguments.iter().map(|a| from_legacy_term(vm, a)));
                vm.rebuild_app_wrapped(wrapped)
            }
        }
    }
}
