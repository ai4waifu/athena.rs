//! 将已解码微积分应用（arena `ExprId`）识别为 [`CalculusRequest`]。

use athena_types::{AssumptionSet, ExprId};

use super::{
    ctx::CalculusCtx,
    request::{CalculusRequest, DerivativeOrder, LimitApproach, LimitDirection},
};
use crate::execution::vm::Shape;

/// 若可识别，将微积分应用映射为域请求。
///
/// 仅语言中立的 AthenaIR 形态 — 方言文本解析留在宿主（SXO）。
pub fn try_calculus_request(cc: &mut CalculusCtx<'_>, root: ExprId) -> Option<CalculusRequest> {
    let (name, args) = cc.app(root)?;
    let args = args.as_slice();
    match name.as_str() {
        "D" => lower_d(cc, args),
        "Integrate" => lower_integrate(cc, args),
        "Limit" => lower_limit(cc, args),
        "Series" => lower_series(cc, args),
        "LaurentSeries" => lower_laurent(cc, args),
        "Asymptotic" => lower_asymptotic(cc, args),
        "Residue" => lower_residue(cc, args),
        "DSolve" => lower_dsolve(cc, args),
        "LaplaceTransform" => lower_laplace(cc, args),
        "FourierTransform" => lower_fourier(cc, args),
        "ZTransform" => lower_z(cc, args),
        "Divergence" => lower_divergence(cc, args),
        "Curl" => lower_curl(cc, args),
        _ => None,
    }
}

fn lower_d(cc: &mut CalculusCtx<'_>, args: &[ExprId]) -> Option<CalculusRequest> {
    match args {
        [expr, var] => {
            if let Some(v) = symbol_name(cc, *var) {
                return Some(CalculusRequest::Derivative {
                    expression: *expr,
                    variable: v,
                    order: DerivativeOrder::First,
                    assumptions: AssumptionSet::empty(),
                });
            }
            if let Some(Shape::List(items)) = cc.shape(*var) {
                if items.len() == 2 {
                    let v = symbol_name(cc, items[0])?;
                    let n = cc.int_exp(items[1])?;
                    let n_u = u32::try_from(n).ok()?;
                    return Some(CalculusRequest::Derivative {
                        expression: *expr,
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

fn lower_integrate(cc: &mut CalculusCtx<'_>, args: &[ExprId]) -> Option<CalculusRequest> {
    match args {
        [expr, var] => {
            if let Some(v) = symbol_name(cc, *var) {
                return Some(CalculusRequest::Integral { expression: *expr, variable: v, assumptions: AssumptionSet::empty() });
            }
            if let Some(Shape::List(items)) = cc.shape(*var) {
                if items.len() == 3 {
                    let v = symbol_name(cc, items[0])?;
                    return Some(CalculusRequest::DefiniteIntegral {
                        expression: *expr,
                        variable: v,
                        lower: items[1],
                        upper: items[2],
                        assumptions: AssumptionSet::empty(),
                    });
                }
            }
            None
        }
        _ => None,
    }
}

fn lower_limit(cc: &mut CalculusCtx<'_>, args: &[ExprId]) -> Option<CalculusRequest> {
    let (expr, variable, approach, direction) = match args {
        [expr, spec] => {
            let (v, approach) = parse_limit_spec(cc, *spec)?;
            (*expr, v, approach, LimitDirection::TwoSided)
        }
        [expr, spec, dir] => {
            let (v, approach) = parse_limit_spec(cc, *spec)?;
            let direction = match symbol_name(cc, *dir).as_deref() {
                Some("FromBelow") | Some("Left") => LimitDirection::FromBelow,
                Some("FromAbove") | Some("Right") => LimitDirection::FromAbove,
                _ => LimitDirection::TwoSided,
            };
            (*expr, v, approach, direction)
        }
        _ => return None,
    };
    Some(CalculusRequest::Limit { expression: expr, variable, approach, direction, assumptions: AssumptionSet::empty() })
}

fn parse_limit_spec(cc: &mut CalculusCtx<'_>, spec: ExprId) -> Option<(String, LimitApproach)> {
    if let Some((h, args)) = cc.app(spec) {
        if h == "Rule" && args.len() == 2 {
            let v = symbol_name(cc, args[0])?;
            return Some((v, approach_from_term(cc, args[1])));
        }
    }
    if let Some(Shape::List(items)) = cc.shape(spec) {
        if items.len() == 2 {
            let v = symbol_name(cc, items[0])?;
            return Some((v, approach_from_term(cc, items[1])));
        }
    }
    None
}

fn approach_from_term(cc: &CalculusCtx<'_>, term: ExprId) -> LimitApproach {
    if is_sym_infinity(cc, term) {
        return LimitApproach::PositiveInfinity;
    }
    if let Some((h, args)) = cc.app(term) {
        if h == "Times" && args.len() == 2 && cc.int_exp(args[0]) == Some(-1) && is_sym_infinity(cc, args[1]) {
            return LimitApproach::NegativeInfinity;
        }
    }
    LimitApproach::Finite(term)
}

fn lower_series(cc: &mut CalculusCtx<'_>, args: &[ExprId]) -> Option<CalculusRequest> {
    let [expr, spec] = args
    else {
        return None;
    };
    let Some(Shape::List(items)) = cc.shape(*spec)
    else {
        return None;
    };
    if items.len() < 2 {
        return None;
    }
    let variable = symbol_name(cc, items[0])?;
    let center = items[1];
    let order = if items.len() >= 3 {
        let n = cc.int_exp(items[2])?;
        u32::try_from(n).ok()?
    }
    else {
        3
    };
    Some(CalculusRequest::Series { expression: *expr, variable, center, order, assumptions: AssumptionSet::empty() })
}

fn lower_laurent(cc: &mut CalculusCtx<'_>, args: &[ExprId]) -> Option<CalculusRequest> {
    let [expr, spec] = args
    else {
        return None;
    };
    let Some(Shape::List(items)) = cc.shape(*spec)
    else {
        return None;
    };
    if items.len() < 2 {
        return None;
    }
    let variable = symbol_name(cc, items[0])?;
    let center = items[1];
    let order = if items.len() >= 3 {
        let n = cc.int_exp(items[2])?;
        u32::try_from(n).ok()?
    }
    else {
        3
    };
    Some(CalculusRequest::Laurent { expression: *expr, variable, center, order, assumptions: AssumptionSet::empty() })
}

fn lower_asymptotic(cc: &mut CalculusCtx<'_>, args: &[ExprId]) -> Option<CalculusRequest> {
    match args {
        [expr, spec] => {
            let Some(Shape::List(items)) = cc.shape(*spec)
            else {
                return None;
            };
            if items.len() < 2 {
                return None;
            }
            let variable = symbol_name(cc, items[0])?;
            if !is_sym_infinity(cc, items[1]) {
                return None;
            }
            let order = if items.len() >= 3 {
                let n = cc.int_exp(items[2])?;
                u32::try_from(n).ok()?
            }
            else {
                3
            };
            Some(CalculusRequest::Asymptotic { expression: *expr, variable, order, assumptions: AssumptionSet::empty() })
        }
        [expr, var, order_term] => {
            let variable = symbol_name(cc, *var)?;
            let n = cc.int_exp(*order_term)?;
            let order = u32::try_from(n).ok()?;
            Some(CalculusRequest::Asymptotic { expression: *expr, variable, order, assumptions: AssumptionSet::empty() })
        }
        _ => None,
    }
}

fn lower_residue(cc: &mut CalculusCtx<'_>, args: &[ExprId]) -> Option<CalculusRequest> {
    match args {
        [expr, spec] => {
            let Some(Shape::List(items)) = cc.shape(*spec)
            else {
                return None;
            };
            if items.len() < 2 {
                return None;
            }
            let variable = symbol_name(cc, items[0])?;
            Some(CalculusRequest::Residue { expression: *expr, variable, point: items[1], assumptions: AssumptionSet::empty() })
        }
        [expr, var, point] => {
            let variable = symbol_name(cc, *var)?;
            Some(CalculusRequest::Residue { expression: *expr, variable, point: *point, assumptions: AssumptionSet::empty() })
        }
        _ => None,
    }
}

fn lower_dsolve(cc: &mut CalculusCtx<'_>, args: &[ExprId]) -> Option<CalculusRequest> {
    let [equation, dep, indep] = args
    else {
        return None;
    };
    let dependent = symbol_name(cc, *dep)?;
    let independent = symbol_name(cc, *indep)?;
    Some(CalculusRequest::SolveOde {
        equation: *equation,
        dependent,
        independent,
        initial: None,
        assumptions: AssumptionSet::empty(),
    })
}

fn lower_laplace(cc: &mut CalculusCtx<'_>, args: &[ExprId]) -> Option<CalculusRequest> {
    let [expression, time, transform] = args
    else {
        return None;
    };
    Some(CalculusRequest::Transform {
        kind: super::request::TransformKind::Laplace,
        expression: *expression,
        time_variable: symbol_name(cc, *time)?,
        transform_variable: symbol_name(cc, *transform)?,
        assumptions: AssumptionSet::empty(),
    })
}

fn lower_fourier(cc: &mut CalculusCtx<'_>, args: &[ExprId]) -> Option<CalculusRequest> {
    let [expression, time, transform] = args
    else {
        return None;
    };
    Some(CalculusRequest::Transform {
        kind: super::request::TransformKind::Fourier,
        expression: *expression,
        time_variable: symbol_name(cc, *time)?,
        transform_variable: symbol_name(cc, *transform)?,
        assumptions: AssumptionSet::empty(),
    })
}

fn lower_z(cc: &mut CalculusCtx<'_>, args: &[ExprId]) -> Option<CalculusRequest> {
    let [expression, time, transform] = args
    else {
        return None;
    };
    Some(CalculusRequest::Transform {
        kind: super::request::TransformKind::Z,
        expression: *expression,
        time_variable: symbol_name(cc, *time)?,
        transform_variable: symbol_name(cc, *transform)?,
        assumptions: AssumptionSet::empty(),
    })
}

fn lower_divergence(cc: &mut CalculusCtx<'_>, args: &[ExprId]) -> Option<CalculusRequest> {
    let [comps, vars] = args
    else {
        return None;
    };
    let Some(Shape::List(components)) = cc.shape(*comps)
    else {
        return None;
    };
    let Some(Shape::List(var_terms)) = cc.shape(*vars)
    else {
        return None;
    };
    let variables: Option<Vec<String>> = var_terms.iter().map(|v| symbol_name(cc, *v)).collect();
    Some(CalculusRequest::Divergence {
        components: components.to_vec(),
        variables: variables?,
        assumptions: AssumptionSet::empty(),
    })
}

fn lower_curl(cc: &mut CalculusCtx<'_>, args: &[ExprId]) -> Option<CalculusRequest> {
    let [comps, vars] = args
    else {
        return None;
    };
    let Some(Shape::List(components)) = cc.shape(*comps)
    else {
        return None;
    };
    let Some(Shape::List(var_terms)) = cc.shape(*vars)
    else {
        return None;
    };
    let variables: Option<Vec<String>> = var_terms.iter().map(|v| symbol_name(cc, *v)).collect();
    Some(CalculusRequest::Curl { components: components.to_vec(), variables: variables?, assumptions: AssumptionSet::empty() })
}

fn is_sym_infinity(cc: &CalculusCtx<'_>, term: ExprId) -> bool {
    matches!(cc.shape(term), Some(Shape::Sym(s)) if cc.sym_is(s, "Infinity"))
}

fn symbol_name(cc: &CalculusCtx<'_>, term: ExprId) -> Option<String> {
    match cc.shape(term)? {
        Shape::Sym(s) => Some(cc.sym_name(s).to_string()),
        _ => None,
    }
}
