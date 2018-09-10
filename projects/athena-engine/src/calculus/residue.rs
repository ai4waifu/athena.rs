//! 复分析留数 — 经 Laurent `(z-a)^{-1}` 系数提取（bootstrap）。

use athena_types::{Diagnostic, DiagnosticCode};

use crate::term::Term;

use super::{
    result::CalculusResult,
    series::{Remainder, laurent},
};

/// 在 `point` 处的留数对象（非裸系数）。
#[derive(Debug, Clone, PartialEq)]
pub struct Residue {
    /// 源表达式。
    pub expression: Term,
    /// 复变量。
    pub variable: String,
    /// 展开点（已解码）。
    pub point: Term,
    /// 留数值（`(z-a)^{-1}` 系数；解析则多为 0）。
    pub value: Term,
    /// 若 Laurent 成功估出的极点阶（主部最低幂的相反数）；解析点为 `0`。
    pub pole_order: u32,
}

impl Residue {
    /// 桥接为留数标量项。
    pub fn to_bridge_term(&self) -> Term {
        self.value.clone()
    }
}

/// 计算 `Res(expression, variable → point)`。
///
/// Bootstrap：对 `point` 做 Laurent（正则部分阶 0），提取 `power == -1` 的系数。
pub fn residue_checked(expression: &Term, variable: &str, point: &Term) -> CalculusResult<Residue> {
    match laurent(expression, variable, point, 0) {
        CalculusResult::Exact { value: series, conditions } => {
            let pole_order =
                series.terms.iter().filter_map(|(_, p)| if *p < 0 { Some((-*p) as u32) } else { None }).max().unwrap_or(0);
            let value = series.terms.iter().find(|(_, p)| *p == -1).map(|(c, _)| c.clone()).unwrap_or_else(|| Term::int(0));
            // 若余项未知且无主部，不假装精确 0
            if matches!(series.remainder, Remainder::Unknown) && pole_order == 0 && is_zero_like(&value) {
                return CalculusResult::Unevaluated {
                    expression: Residue {
                        expression: expression.clone(),
                        variable: variable.to_string(),
                        point: point.clone(),
                        value: Term::apply(
                            "Residue",
                            vec![expression.clone(), Term::List(vec![Term::symbol(variable), point.clone()])],
                        ),
                        pole_order: 0,
                    },
                    reason: Diagnostic::new(DiagnosticCode::SeriesRemainderUnknown),
                };
            }
            let _ = conditions;
            CalculusResult::Exact {
                value: Residue {
                    expression: expression.clone(),
                    variable: variable.to_string(),
                    point: point.clone(),
                    value,
                    pole_order,
                },
                conditions: Vec::new(),
            }
        }
        CalculusResult::Conditional { value: series, conditions } => {
            let pole_order =
                series.terms.iter().filter_map(|(_, p)| if *p < 0 { Some((-*p) as u32) } else { None }).max().unwrap_or(0);
            let value = series.terms.iter().find(|(_, p)| *p == -1).map(|(c, _)| c.clone()).unwrap_or_else(|| Term::int(0));
            CalculusResult::Conditional {
                value: Residue {
                    expression: expression.clone(),
                    variable: variable.to_string(),
                    point: point.clone(),
                    value,
                    pole_order,
                },
                conditions,
            }
        }
        CalculusResult::Unevaluated { .. } => CalculusResult::Unevaluated {
            expression: Residue {
                expression: expression.clone(),
                variable: variable.to_string(),
                point: point.clone(),
                value: Term::apply(
                    "Residue",
                    vec![expression.clone(), Term::List(vec![Term::symbol(variable), point.clone()])],
                ),
                pole_order: 0,
            },
            reason: Diagnostic::new(DiagnosticCode::SeriesRemainderUnknown),
        },
    }
}

fn is_zero_like(term: &Term) -> bool {
    matches!(term, Term::Atom(crate::term::Atom::Number(n)) if n.is_zero())
}
