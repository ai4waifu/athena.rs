//! 将已解码微积分 [`Term`] 头部识别为 [`CalculusRequest`]。

use athena_types::AssumptionSet;

use crate::term::{Atom, Term, number_from_term};

use super::request::{CalculusRequest, DerivativeOrder, LimitApproach, LimitDirection};

/// 若可识别，将桥接 [`Term`] 应用映射为微积分域请求。
///
/// 仅语言中立的 Term 形态 — 方言文本解析留在宿主（SXO）。
pub fn try_calculus_request(term: &Term) -> Option<CalculusRequest> {
    let Term::Application { head, arguments: args } = term
    else {
        return None;
    };
    let name = head.head_name()?;
    match name {
        "D" => lower_d(args),
        "Integrate" => lower_integrate(args),
        "Limit" => lower_limit(args),
        "Series" => lower_series(args),
        "LaurentSeries" => lower_laurent(args),
        "Asymptotic" => lower_asymptotic(args),
        "DSolve" => lower_dsolve(args),
        "LaplaceTransform" => lower_laplace(args),
        "FourierTransform" => lower_fourier(args),
        "ZTransform" => lower_z(args),
        "Divergence" => lower_divergence(args),
        "Curl" => lower_curl(args),
        _ => None,
    }
}

fn lower_d(args: &[Term]) -> Option<CalculusRequest> {
    match args {
        [expr, var] => {
            if let Some(v) = symbol_name(var) {
                return Some(CalculusRequest::Derivative {
                    expression: expr.clone(),
                    variable: v,
                    order: DerivativeOrder::First,
                    assumptions: AssumptionSet::empty(),
                });
            }
            if let Term::List(items) = var {
                if items.len() == 2 {
                    let v = symbol_name(&items[0])?;
                    let n = number_from_term(&items[1]).and_then(|e| e.as_integer_exp())?;
                    let n_u = u32::try_from(&n).ok()?;
                    return Some(CalculusRequest::Derivative {
                        expression: expr.clone(),
                        variable: v,
                        order: if n_u <= 1 { DerivativeOrder::First } else { DerivativeOrder::Repeated(n_u) },
                        assumptions: AssumptionSet::empty(),
                    });
                }
            }
            None
        }
        _ => None,
    }
}

fn lower_integrate(args: &[Term]) -> Option<CalculusRequest> {
    match args {
        [expr, var] => {
            if let Some(v) = symbol_name(var) {
                return Some(CalculusRequest::Integral {
                    expression: expr.clone(),
                    variable: v,
                    assumptions: AssumptionSet::empty(),
                });
            }
            if let Term::List(items) = var {
                if items.len() == 3 {
                    let v = symbol_name(&items[0])?;
                    return Some(CalculusRequest::DefiniteIntegral {
                        expression: expr.clone(),
                        variable: v,
                        lower: items[1].clone(),
                        upper: items[2].clone(),
                        assumptions: AssumptionSet::empty(),
                    });
                }
            }
            None
        }
        _ => None,
    }
}

fn lower_limit(args: &[Term]) -> Option<CalculusRequest> {
    let (expr, variable, approach, direction) = match args {
        [expr, spec] => {
            let (v, approach) = parse_limit_spec(spec)?;
            (expr.clone(), v, approach, LimitDirection::TwoSided)
        }
        [expr, spec, dir] => {
            let (v, approach) = parse_limit_spec(spec)?;
            let direction = match symbol_name(dir).as_deref() {
                Some("FromBelow") | Some("Left") => LimitDirection::FromBelow,
                Some("FromAbove") | Some("Right") => LimitDirection::FromAbove,
                _ => LimitDirection::TwoSided,
            };
            (expr.clone(), v, approach, direction)
        }
        _ => return None,
    };
    Some(CalculusRequest::Limit { expression: expr, variable, approach, direction, assumptions: AssumptionSet::empty() })
}

fn parse_limit_spec(spec: &Term) -> Option<(String, LimitApproach)> {
    match spec {
        Term::Application { head, arguments: args } if head.is_symbol("Rule") && args.len() == 2 => {
            let v = symbol_name(&args[0])?;
            Some((v, approach_from_term(&args[1])))
        }
        Term::List(items) if items.len() == 2 => {
            let v = symbol_name(&items[0])?;
            Some((v, approach_from_term(&items[1])))
        }
        _ => None,
    }
}

fn approach_from_term(term: &Term) -> LimitApproach {
    if term.is_symbol("Infinity") {
        return LimitApproach::PositiveInfinity;
    }
    if let Term::Application { head, arguments: args } = term {
        if head.is_symbol("Times")
            && args.len() == 2
            && number_from_term(&args[0]).is_some_and(|n| n.as_integer_exp() == Some((-1).into()))
            && args[1].is_symbol("Infinity")
        {
            return LimitApproach::NegativeInfinity;
        }
    }
    LimitApproach::Finite(term.clone())
}

fn lower_series(args: &[Term]) -> Option<CalculusRequest> {
    let [expr, spec] = args
    else {
        return None;
    };
    let Term::List(items) = spec
    else {
        return None;
    };
    if items.len() < 2 {
        return None;
    }
    let variable = symbol_name(&items[0])?;
    let center = items[1].clone();
    let order = if items.len() >= 3 {
        let n = number_from_term(&items[2]).and_then(|e| e.as_integer_exp())?;
        u32::try_from(&n).ok()?
    }
    else {
        3
    };
    Some(CalculusRequest::Series { expression: expr.clone(), variable, center, order, assumptions: AssumptionSet::empty() })
}

fn lower_laurent(args: &[Term]) -> Option<CalculusRequest> {
    // LaurentSeries[expr, {x, c, n}]
    let [expr, spec] = args
    else {
        return None;
    };
    let Term::List(items) = spec
    else {
        return None;
    };
    if items.len() < 2 {
        return None;
    }
    let variable = symbol_name(&items[0])?;
    let center = items[1].clone();
    let order = if items.len() >= 3 {
        let n = number_from_term(&items[2]).and_then(|e| e.as_integer_exp())?;
        u32::try_from(&n).ok()?
    } else {
        3
    };
    Some(CalculusRequest::Laurent {
        expression: expr.clone(),
        variable,
        center,
        order,
        assumptions: AssumptionSet::empty(),
    })
}

fn lower_asymptotic(args: &[Term]) -> Option<CalculusRequest> {
    // Asymptotic[expr, {x, Infinity, n}] 或 Asymptotic[expr, x, n]
    match args {
        [expr, spec] => {
            let Term::List(items) = spec
            else {
                return None;
            };
            if items.len() < 2 {
                return None;
            }
            let variable = symbol_name(&items[0])?;
            if !items[1].is_symbol("Infinity") {
                return None;
            }
            let order = if items.len() >= 3 {
                let n = number_from_term(&items[2]).and_then(|e| e.as_integer_exp())?;
                u32::try_from(&n).ok()?
            } else {
                3
            };
            Some(CalculusRequest::Asymptotic {
                expression: expr.clone(),
                variable,
                order,
                assumptions: AssumptionSet::empty(),
            })
        }
        [expr, var, order_term] => {
            let variable = symbol_name(var)?;
            let n = number_from_term(order_term).and_then(|e| e.as_integer_exp())?;
            let order = u32::try_from(&n).ok()?;
            Some(CalculusRequest::Asymptotic {
                expression: expr.clone(),
                variable,
                order,
                assumptions: AssumptionSet::empty(),
            })
        }
        _ => None,
    }
}

fn lower_dsolve(args: &[Term]) -> Option<CalculusRequest> {
    let [equation, dep, indep] = args
    else {
        return None;
    };
    let dependent = symbol_name(dep)?;
    let independent = symbol_name(indep)?;
    Some(CalculusRequest::SolveOde {
        equation: equation.clone(),
        dependent,
        independent,
        initial: None,
        assumptions: AssumptionSet::empty(),
    })
}

fn lower_laplace(args: &[Term]) -> Option<CalculusRequest> {
    // LaplaceTransform[expr, t, s]
    let [expression, time, transform] = args
    else {
        return None;
    };
    Some(CalculusRequest::Transform {
        kind: super::request::TransformKind::Laplace,
        expression: expression.clone(),
        time_variable: symbol_name(time)?,
        transform_variable: symbol_name(transform)?,
        assumptions: AssumptionSet::empty(),
    })
}

fn lower_fourier(args: &[Term]) -> Option<CalculusRequest> {
    // FourierTransform[expr, t, ω]
    let [expression, time, transform] = args
    else {
        return None;
    };
    Some(CalculusRequest::Transform {
        kind: super::request::TransformKind::Fourier,
        expression: expression.clone(),
        time_variable: symbol_name(time)?,
        transform_variable: symbol_name(transform)?,
        assumptions: AssumptionSet::empty(),
    })
}

fn lower_z(args: &[Term]) -> Option<CalculusRequest> {
    // ZTransform[expr, n, z]
    let [expression, time, transform] = args
    else {
        return None;
    };
    Some(CalculusRequest::Transform {
        kind: super::request::TransformKind::Z,
        expression: expression.clone(),
        time_variable: symbol_name(time)?,
        transform_variable: symbol_name(transform)?,
        assumptions: AssumptionSet::empty(),
    })
}

fn lower_divergence(args: &[Term]) -> Option<CalculusRequest> {
    // Divergence[{F1,…}, {x1,…}]
    let [comps, vars] = args
    else {
        return None;
    };
    let Term::List(components) = comps
    else {
        return None;
    };
    let Term::List(var_terms) = vars
    else {
        return None;
    };
    let variables: Option<Vec<String>> = var_terms.iter().map(symbol_name).collect();
    Some(CalculusRequest::Divergence {
        components: components.clone(),
        variables: variables?,
        assumptions: AssumptionSet::empty(),
    })
}

fn lower_curl(args: &[Term]) -> Option<CalculusRequest> {
    // Curl[{Fx,Fy,Fz}, {x,y,z}]
    let [comps, vars] = args
    else {
        return None;
    };
    let Term::List(components) = comps
    else {
        return None;
    };
    let Term::List(var_terms) = vars
    else {
        return None;
    };
    let variables: Option<Vec<String>> = var_terms.iter().map(symbol_name).collect();
    Some(CalculusRequest::Curl { components: components.clone(), variables: variables?, assumptions: AssumptionSet::empty() })
}

fn symbol_name(term: &Term) -> Option<String> {
    match term {
        Term::Atom(Atom::Symbol(s)) => Some(s.clone()),
        _ => None,
    }
}
