//! Solve 域执行：一元方程 → 多项式根 → 规则列表项。

use std::collections::BTreeMap;

use athena_ir::{ApplicationHead, SemanticOperator};
use athena_numeric::{Number, add as num_add, div as num_div, mul as num_mul};
use athena_types::{Diagnostic, DiagnosticCode, Result, SourceSpan, SymbolId, TermId};

use super::{
    BoundSymbol, Constraint, SolveDomain, SolveGoal, SolvePolicy, SolveRequest, SolveResult,
    assemble_solve_problem, execute_polynomial_root_goal, normalize_relational_application, value_table::BindingValue, ExecutionLimits,
};
use crate::{
    domains::{
        context::DomainExecutionContext,
        polynomial::{CoefficientDomain, MonomialOrder, PolynomialBuilder, PolynomialFactorLimits},
    },
    execution::{push_number, push_semantic},
    runtime::{
        session::Session,
        values::{
            arena::{push_extension, push_list},
            numeric_clone::clone_number,
        },
    },
};

/// 执行 Solve 域请求。
pub fn execute_solve(session: &mut Session, request: SolveRequest) -> SolveResult {
    match request {
        SolveRequest::UnivariateEquation { equation, unknown } => match solve_univariate_equation(session, equation, unknown) {
            Ok((term, coverage)) => SolveResult::Exact { term, coverage },
            Err(reason) => SolveResult::Unevaluated {
                expression: echo_solve(session, equation, unknown),
                reason,
            },
        },
    }
}

fn echo_solve(session: &mut Session, equation: TermId, unknown: SymbolId) -> TermId {
    let var = session.builder().symbol_id(unknown, SourceSpan::default());
    let ext = session.extensions.intern("Solve");
    push_extension(session, ext, vec![equation, var])
}

fn solve_univariate_equation(session: &mut Session, equation: TermId, unknown: SymbolId) -> Result<(TermId, super::CoverageStatus)> {
    let poly_term = equation_to_zero_form(session, equation)?;
    let coeffs = {
        let mut dc = DomainExecutionContext::new(session);
        let folded = dc.fold_term(poly_term)?;
        collect_coeffs(&dc, folded, unknown)?
    };
    if coeffs.is_empty() {
        return Err(diag("empty_polynomial"));
    }
    let ring = session.rings.intern(CoefficientDomain::Rational, vec![unknown], MonomialOrder::Lex)?;
    let mut builder = PolynomialBuilder::new(ring);
    for (exp, coef) in coeffs {
        builder.push_term(coef, vec![exp])?;
    }
    let polynomial = builder.build(&session.rings)?;
    let problem = assemble_solve_problem(
        &session.arena,
        &[equation],
        vec![BoundSymbol::free(unknown)],
        Vec::new(),
        SolveDomain::Rationals,
        athena_types::AssumptionSetId(0),
        SolveGoal::PolynomialRootSet,
        SolvePolicy::default(),
        ExecutionLimits::default(),
    )?;
    let adapted = execute_polynomial_root_goal(&problem, polynomial, &session.rings, PolynomialFactorLimits::default())?;
    let coverage = adapted.solution.coverage.owning_copy();
    if matches!(coverage, super::CoverageStatus::Unsupported | super::CoverageStatus::Invalid) && adapted.solution.branches.is_empty() {
        return Err(diag("unsupported_root_set"));
    }
    let term = materialize_solution_rules(session, &adapted.solution, &adapted.values)?;
    Ok((term, coverage))
}

fn equation_to_zero_form(session: &mut Session, equation: TermId) -> Result<TermId> {
    match normalize_relational_application(&session.arena, equation)? {
        Constraint::Equation(eq) => {
            let mut dc = DomainExecutionContext::new(session);
            let neg = dc.apply_semantic(SemanticOperator::Multiply, vec![dc.in_(-1), eq.rhs]);
            dc.fold_term(dc.apply_semantic(SemanticOperator::Add, vec![eq.lhs, neg]))
        }
        _ => Err(diag("expected_equation")),
    }
}

fn collect_coeffs(dc: &DomainExecutionContext<'_>, expr: TermId, var: SymbolId) -> Result<BTreeMap<u32, Number>> {
    use crate::execution::shape::Shape;
    let mut out = BTreeMap::new();
    match dc.shape(expr) {
        Some(Shape::Number) => {
            accum(&mut out, 0, dc.number_of(expr).expect("number"))?;
        }
        Some(Shape::Symbol(s)) if dc.symbol_id_is(s, var) => {
            accum(&mut out, 1, Number::small_int(1))?;
        }
        Some(Shape::Symbol(_)) | Some(Shape::Constant(_)) => return Err(diag("non_univariate_symbol")),
        Some(Shape::Application(head, args)) => match head {
            ApplicationHead::Semantic(SemanticOperator::Add) => {
                for a in args {
                    merge_into(&mut out, collect_coeffs(dc, a, var)?)?;
                }
            }
            ApplicationHead::Semantic(SemanticOperator::Subtract) if args.len() == 2 => {
                merge_into(&mut out, collect_coeffs(dc, args[0], var)?)?;
                merge_into(&mut out, scale_map(collect_coeffs(dc, args[1], var)?, Number::small_int(-1))?)?;
            }
            ApplicationHead::Semantic(SemanticOperator::Negate) if args.len() == 1 => {
                merge_into(&mut out, scale_map(collect_coeffs(dc, args[0], var)?, Number::small_int(-1))?)?;
            }
            ApplicationHead::Semantic(SemanticOperator::Multiply) => {
                let mut acc = BTreeMap::from([(0u32, Number::small_int(1))]);
                for a in args {
                    acc = convolve(&acc, &collect_coeffs(dc, a, var)?)?;
                }
                out = acc;
            }
            ApplicationHead::Semantic(SemanticOperator::Power) if args.len() == 2 => {
                if is_var_symbol(dc, args[0], var) {
                    let Some(n) = dc.int_exp(args[1])
                    else {
                        return Err(diag("non_integer_power"));
                    };
                    if n < 0 {
                        return Err(diag("negative_power"));
                    }
                    accum(&mut out, n as u32, Number::small_int(1))?;
                }
                else if let Some(base_n) = dc.number_of(args[0]) {
                    let Some(e) = dc.int_exp(args[1])
                    else {
                        return Err(diag("non_integer_power"));
                    };
                    if e < 0 {
                        return Err(diag("negative_numeric_power"));
                    }
                    let mut p = Number::small_int(1);
                    for _ in 0..e {
                        p = num_mul(clone_number(&p), clone_number(&base_n)).map_err(|_| diag("pow_overflow"))?;
                    }
                    accum(&mut out, 0, p)?;
                }
                else {
                    return Err(diag("unsupported_power"));
                }
            }
            ApplicationHead::Semantic(SemanticOperator::Divide) if args.len() == 2 => {
                let Some(den_n) = dc.number_of(args[1])
                else {
                    return Err(diag("non_numeric_denominator"));
                };
                for (e, c) in collect_coeffs(dc, args[0], var)? {
                    let q = num_div(clone_number(&c), clone_number(&den_n)).map_err(|_| diag("div_failed"))?;
                    accum(&mut out, e, q)?;
                }
            }
            _ => return Err(diag("unsupported_term")),
        },
        _ => return Err(diag("unsupported_term")),
    }
    Ok(out)
}

fn is_var_symbol(dc: &DomainExecutionContext<'_>, term: TermId, var: SymbolId) -> bool {
    matches!(dc.shape(term), Some(crate::execution::shape::Shape::Symbol(s)) if dc.symbol_id_is(s, var))
}

fn accum(map: &mut BTreeMap<u32, Number>, exp: u32, coef: Number) -> Result<()> {
    let entry = map.entry(exp).or_insert_with(|| Number::small_int(0));
    *entry = num_add(clone_number(entry), coef).map_err(|_| diag("add_overflow"))?;
    Ok(())
}

fn merge_into(dst: &mut BTreeMap<u32, Number>, src: BTreeMap<u32, Number>) -> Result<()> {
    for (e, c) in src {
        accum(dst, e, c)?;
    }
    Ok(())
}

fn scale_map(src: BTreeMap<u32, Number>, scale: Number) -> Result<BTreeMap<u32, Number>> {
    let mut out = BTreeMap::new();
    for (e, c) in src {
        let v = num_mul(clone_number(&c), clone_number(&scale)).map_err(|_| diag("mul_overflow"))?;
        accum(&mut out, e, v)?;
    }
    Ok(out)
}

fn convolve(a: &BTreeMap<u32, Number>, b: &BTreeMap<u32, Number>) -> Result<BTreeMap<u32, Number>> {
    let mut out = BTreeMap::new();
    for (ea, ca) in a {
        for (eb, cb) in b {
            let prod = num_mul(clone_number(ca), clone_number(cb)).map_err(|_| diag("mul_overflow"))?;
            accum(&mut out, ea + eb, prod)?;
        }
    }
    Ok(out)
}

/// 将 [`SolutionSet`] 分支物化为 `{{v -> …}, …}` 规则列表（多未知量按变量序并列 Rule）。
fn materialize_solution_rules(
    session: &mut Session,
    solution: &super::SolutionSet,
    values: &super::value_table::BindingValueTable,
) -> Result<TermId> {
    let mut sortable: Vec<(String, TermId)> = Vec::new();
    for branch in &solution.branches {
        let mut rules = Vec::new();
        let mut sort_parts = Vec::new();
        for var in &solution.variables {
            let Some(binding_tid) = branch.bindings.get(var)
            else {
                continue;
            };
            let Some(value) = values.get(binding_tid)
            else {
                continue;
            };
            let value_term = binding_value_to_term(session, value);
            sort_parts.push(binding_sort_key(value));
            let sym = session.builder().symbol_id(var.symbol, SourceSpan::default());
            rules.push(push_semantic(session, SemanticOperator::Rule, vec![sym, value_term]));
        }
        if rules.is_empty() {
            continue;
        }
        sortable.push((sort_parts.join("|"), push_list(session, rules)));
    }
    sortable.sort_by(|a, b| a.0.cmp(&b.0));
    Ok(push_list(session, sortable.into_iter().map(|(_, t)| t).collect()))
}

fn binding_sort_key(value: &BindingValue) -> String {
    match value {
        BindingValue::Number(n) => n.to_render_string(),
        BindingValue::Rational(r) => r.to_wire_string(),
        BindingValue::MachineF64(x) => format!("{x}"),
    }
}

fn binding_value_to_term(session: &mut Session, value: &BindingValue) -> TermId {
    match value {
        BindingValue::Number(n) => push_number(session, clone_number(n)),
        BindingValue::Rational(r) => {
            push_number(session, Number::from_rational_normalized(crate::runtime::values::numeric_clone::clone_rational(r)))
        }
        BindingValue::MachineF64(x) => push_number(session, Number::machine(*x)),
    }
}

fn diag(reason: &str) -> Diagnostic {
    Diagnostic::new(DiagnosticCode::UnsupportedOperation)
        .detail("domain", "solve")
        .detail("operation", "univariate_equation")
        .detail("reason", reason)
}
