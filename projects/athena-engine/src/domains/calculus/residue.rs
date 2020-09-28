//! 复分析留数 — 经 Laurent `(z-a)^{-1}` 系数提取（引导实现 · arena 版 ）。

use athena_ir::{ApplicationHead, SemanticOperator};
use athena_types::{Diagnostic, DiagnosticCode, Result, SymbolId, TermId};

use super::{
    derivative::differentiate,
    result::CalculusResult,
    series::{Remainder, laurent},
    symbol_rewrite::replace_symbol,
};
use crate::domains::context::DomainExecutionContext;

/// 在 `point` 处的留数对象（非裸系数）。
#[derive(Debug, PartialEq)]
pub struct Residue {
    /// 源表达式。
    pub expression: TermId,
    /// 复变量。
    pub variable: SymbolId,
    /// 展开点（已解码）。
    pub point: TermId,
    /// 留数值（`(z-a)^{-1}` 系数；解析则多为 0）。
    pub value: TermId,
    /// 若 Laurent 成功估出的极点阶（主部最低幂的相反数）；解析点为 `0`。
    pub pole_order: u32,
}

impl Residue {
    /// 桥接为留数标量项。
    pub fn materialize_expression(&self) -> TermId {
        self.value
    }
}

/// 计算 `Res(expression, variable → point)`。
///
/// 引导实现：先尝试简单极点商式 `f/g`（含 `1/g`）；否则对 `point` 做 Laurent（正则部分阶 0），提取 `power == -1` 的系数。
pub fn residue_checked(cc: &mut DomainExecutionContext<'_>, expression: TermId, variable: SymbolId, point: TermId) -> Result<CalculusResult<Residue>> {
    if let Some(value) = try_simple_pole_quotient(cc, expression, variable, point)? {
        return Ok(CalculusResult::Exact {
            value: Residue { expression, variable, point, value, pole_order: 1 },
            conditions: Vec::new(),
        });
    }
    let zero = cc.in_(0);
    Ok(match laurent(cc, expression, variable, point, 0)? {
        CalculusResult::Exact { value: series, conditions } => {
            let pole_order = series.terms.iter().filter_map(|(_, p)| if *p < 0 { Some((-*p) as u32) } else { None }).max().unwrap_or(0);
            let value = series.terms.iter().find(|(_, p)| *p == -1).map(|(c, _)| *c).unwrap_or(zero);
            // 若余项未知且无主部，不假装精确 0
            if matches!(series.remainder, Remainder::Unknown) && pole_order == 0 && is_zero_like(cc, value) {
                return Ok(CalculusResult::Unevaluated {
                    expression: Residue { expression, variable, point, value: residue_echo(cc, expression, variable, point), pole_order: 0 },
                    reason: Diagnostic::new(DiagnosticCode::SeriesRemainderUnknown),
                });
            }
            // Singular at `point` but Laurent returned a regular Exact with no principal part → do not fake 0.
            if pole_order == 0 && is_zero_like(cc, value) && is_singular_at_point(cc, expression, variable, point)? {
                return Ok(CalculusResult::Unevaluated {
                    expression: Residue { expression, variable, point, value: residue_echo(cc, expression, variable, point), pole_order: 0 },
                    reason: Diagnostic::new(DiagnosticCode::SeriesRemainderUnknown),
                });
            }
            let _ = conditions;
            CalculusResult::Exact { value: Residue { expression, variable, point, value, pole_order }, conditions: Vec::new() }
        }
        CalculusResult::Conditional { value: series, conditions } => {
            let pole_order = series.terms.iter().filter_map(|(_, p)| if *p < 0 { Some((-*p) as u32) } else { None }).max().unwrap_or(0);
            let value = series.terms.iter().find(|(_, p)| *p == -1).map(|(c, _)| *c).unwrap_or(zero);
            CalculusResult::Conditional { value: Residue { expression, variable, point, value, pole_order }, conditions }
        }
        CalculusResult::Unevaluated { .. } => CalculusResult::Unevaluated {
            expression: Residue { expression, variable, point, value: residue_echo(cc, expression, variable, point), pole_order: 0 },
            reason: Diagnostic::new(DiagnosticCode::SeriesRemainderUnknown),
        },
    })
}

/// Simple pole: `f/g` or `f·g^{-1}` with `g(a)=0`, `g'(a)≠0` → residue `f(a)/g'(a)`.
///
/// Covers `1/z`, `1/(z-1)`, and `Exp[z]/z` (not only exact reciprocal `1/g`).
fn try_simple_pole_quotient(
    cc: &mut DomainExecutionContext<'_>,
    expression: TermId,
    variable: SymbolId,
    point: TermId,
) -> Result<Option<TermId>> {
    let Some((num, den)) = quotient_parts(cc, expression)
    else {
        return Ok(None);
    };
    let den_at = cc.fold_term(replace_symbol(cc, den, variable, point))?;
    if !is_zero_like(cc, den_at) {
        return Ok(None);
    }
    let num_at = cc.fold_term(replace_symbol(cc, num, variable, point))?;
    if looks_singular_value(cc, num_at) {
        return Ok(None);
    }
    let deriv = differentiate(cc, den, variable)?;
    let d_at = cc.fold_term(replace_symbol(cc, deriv, variable, point))?;
    if is_zero_like(cc, d_at) || looks_singular_value(cc, d_at) || contains_open_head(cc, d_at) {
        return Ok(None);
    }
    if is_zero_like(cc, num_at) {
        return Ok(Some(cc.in_(0)));
    }
    if cc.number_of(d_at).is_some_and(|n| n.is_one()) {
        return Ok(Some(num_at));
    }
    let inv = cc.apply_semantic(SemanticOperator::Power, vec![d_at, cc.in_(-1)]);
    let prod = cc.apply_semantic(SemanticOperator::Multiply, vec![num_at, inv]);
    Ok(Some(cc.fold_term(prod)?))
}

/// Split `f/g`, `g^{-1}`, or `f·g^{-1}` (exactly one reciprocal power factor) into `(f, g)`.
fn quotient_parts(cc: &mut DomainExecutionContext<'_>, expression: TermId) -> Option<(TermId, TermId)> {
    match cc.application_head(expression) {
        Some((ApplicationHead::Semantic(SemanticOperator::Divide), args)) if args.len() == 2 => Some((args[0], args[1])),
        Some((ApplicationHead::Semantic(SemanticOperator::Power), args))
            if args.len() == 2 && cc.int_exp(args[1]) == Some(-1) =>
        {
            Some((cc.in_(1), args[0]))
        }
        Some((ApplicationHead::Semantic(SemanticOperator::Multiply), args)) if !args.is_empty() => {
            let mut reciprocal: Option<TermId> = None;
            let mut num_factors = Vec::new();
            for arg in args {
                match cc.application_head(arg) {
                    Some((ApplicationHead::Semantic(SemanticOperator::Power), pargs))
                        if pargs.len() == 2 && cc.int_exp(pargs[1]) == Some(-1) =>
                    {
                        if reciprocal.is_some() {
                            return None;
                        }
                        reciprocal = Some(pargs[0]);
                    }
                    _ => num_factors.push(arg),
                }
            }
            let den = reciprocal?;
            let num = match num_factors.as_slice() {
                [] => cc.in_(1),
                [only] => *only,
                many => cc.apply_semantic(SemanticOperator::Multiply, many.to_vec()),
            };
            Some((num, den))
        }
        _ => None,
    }
}

fn is_singular_at_point(cc: &mut DomainExecutionContext<'_>, expression: TermId, variable: SymbolId, point: TermId) -> Result<bool> {
    let at = cc.fold_term(replace_symbol(cc, expression, variable, point))?;
    Ok(looks_singular_value(cc, at) || contains_open_head(cc, at))
}

fn looks_singular_value(cc: &DomainExecutionContext<'_>, term: TermId) -> bool {
    match cc.application_head(term) {
        Some((ApplicationHead::Semantic(SemanticOperator::Indeterminate), _)) => true,
        Some((ApplicationHead::Semantic(SemanticOperator::Power), args))
            if args.len() == 2 && is_zero_like(cc, args[0]) && !is_zero_like(cc, args[1]) =>
        {
            true
        }
        Some((ApplicationHead::Semantic(SemanticOperator::Divide), args)) if args.len() == 2 && is_zero_like(cc, args[1]) => true,
        _ => false,
    }
}

fn contains_open_head(cc: &DomainExecutionContext<'_>, term: TermId) -> bool {
    match cc.application_head(term) {
        Some((ApplicationHead::Semantic(op), args)) => {
            if matches!(
                op,
                SemanticOperator::Residue | SemanticOperator::Integrate | SemanticOperator::Limit | SemanticOperator::Series
            ) {
                return true;
            }
            args.iter().any(|a| contains_open_head(cc, *a))
        }
        Some((_, args)) => args.iter().any(|a| contains_open_head(cc, *a)),
        None => false,
    }
}

fn residue_echo(cc: &mut DomainExecutionContext<'_>, expression: TermId, variable: SymbolId, point: TermId) -> TermId {
    let spec = cc.ordered(vec![cc.symbol_id(variable), point]);
    cc.apply_semantic(SemanticOperator::Residue, vec![expression, spec])
}

fn is_zero_like(cc: &DomainExecutionContext<'_>, term: TermId) -> bool {
    cc.number_of(term).is_some_and(|n| n.is_zero())
}
