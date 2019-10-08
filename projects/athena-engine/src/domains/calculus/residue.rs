//! 复分析留数 — 经 Laurent `(z-a)^{-1}` 系数提取（引导实现 · arena 版 Living `25`）。

use athena_ir::SemanticOperator;
use athena_types::{SymbolId, Diagnostic, DiagnosticCode, TermId};

use super::{
    result::CalculusResult,
    series::{Remainder, laurent},
};
use crate::domains::context::DomainExecutionContext;

/// 在 `point` 处的留数对象（非裸系数）。
#[derive(Debug, PartialEq)]
pub struct Residue {
    /// 源表达式。
    pub expression: TermId,
    /// 复变量。
    pub variable: String,
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
/// 引导实现：对 `point` 做 Laurent（正则部分阶 0），提取 `power == -1` 的系数。
pub fn residue_checked(cc: &mut DomainExecutionContext<'_>, expression: TermId, variable: SymbolId, point: TermId) -> CalculusResult<Residue> {
    let zero = cc.in_(0);
    match laurent(cc, expression, variable, point, 0) {
        CalculusResult::Exact { value: series, conditions } => {
            let pole_order = series.terms.iter().filter_map(|(_, p)| if *p < 0 { Some((-*p) as u32) } else { None }).max().unwrap_or(0);
            let value = series.terms.iter().find(|(_, p)| *p == -1).map(|(c, _)| *c).unwrap_or(zero);
            // 若余项未知且无主部，不假装精确 0
            if matches!(series.remainder, Remainder::Unknown) && pole_order == 0 && is_zero_like(cc, value) {
                return CalculusResult::Unevaluated {
                    expression: Residue {
                        expression,
                        variable: cc.symbol_resolve(variable).to_string(),
                        point,
                        value: residue_echo(cc, expression, variable, point),
                        pole_order: 0,
                    },
                    reason: Diagnostic::new(DiagnosticCode::SeriesRemainderUnknown),
                };
            }
            let _ = conditions;
            CalculusResult::Exact {
                value: Residue { expression, variable: cc.symbol_resolve(variable).to_string(), point, value, pole_order },
                conditions: Vec::new(),
            }
        }
        CalculusResult::Conditional { value: series, conditions } => {
            let pole_order = series.terms.iter().filter_map(|(_, p)| if *p < 0 { Some((-*p) as u32) } else { None }).max().unwrap_or(0);
            let value = series.terms.iter().find(|(_, p)| *p == -1).map(|(c, _)| *c).unwrap_or(zero);
            CalculusResult::Conditional { value: Residue { expression, variable: cc.symbol_resolve(variable).to_string(), point, value, pole_order }, conditions }
        }
        CalculusResult::Unevaluated { .. } => CalculusResult::Unevaluated {
            expression: Residue {
                expression,
                variable: cc.symbol_resolve(variable).to_string(),
                point,
                value: residue_echo(cc, expression, variable, point),
                pole_order: 0,
            },
            reason: Diagnostic::new(DiagnosticCode::SeriesRemainderUnknown),
        },
    }
}

fn residue_echo(cc: &mut DomainExecutionContext<'_>, expression: TermId, variable: SymbolId, point: TermId) -> TermId {
    let spec = cc.ordered(vec![cc.symbol_id(variable), point]);
    cc.apply_semantic(SemanticOperator::Residue, vec![expression, spec])
}

fn is_zero_like(cc: &DomainExecutionContext<'_>, term: TermId) -> bool {
    cc.number_of(term).is_some_and(|n| n.is_zero())
}
