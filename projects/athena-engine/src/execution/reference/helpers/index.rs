//! 中立 `IndexSpec` 求值（Reference 与 `ExecutionHost` 共用）。

use athena_types::{Diagnostic, IndexSpec, IntegerIndex, IntegerOffset, Result, TermId};

use super::expand_span_3;
use crate::runtime::{session::Session, values::arena::push_list};

/// 单轴索引步骤结果。
#[derive(Debug)]
pub(crate) enum IndexStep {
    /// 继续下一轴 / 返回该项。
    Next(TermId),
    /// 无法索引，保留原项。
    Residual,
    /// 非法下标（带回声项与诊断）。
    Invalid { echo: TermId, diagnostic: Diagnostic },
}

/// 多轴索引结果（供 host / Reference 映射到槽）。
#[derive(Debug)]
pub(crate) enum IndexOutcome {
    /// 成功或残差项。
    Term(TermId),
    /// 非法下标。
    Invalid { echo: TermId, diagnostic: Diagnostic },
}

/// 应用一条 [`IndexSpec`] 轴（1-based 标量、`All`、`EndRelative`、`Range`）。
pub(crate) fn index_one(session: &mut Session, expr: TermId, spec: &IndexSpec) -> Result<IndexStep> {
    let items = match session.arena.get(expr) {
        Some(athena_ir::TermNode::Collection { elements: items, .. }) => items.clone(),
        Some(athena_ir::TermNode::Application { arguments, .. }) => arguments.clone(),
        _ => return Ok(IndexStep::Residual),
    };
    let len = items.len();

    match spec {
        IndexSpec::All => Ok(IndexStep::Next(push_list(session, items))),
        IndexSpec::EndRelative(IntegerOffset(off)) => {
            let pos = len as i64 + *off - 1;
            if pos < 0 || pos as usize >= len {
                return Ok(IndexStep::Invalid { echo: expr, diagnostic: crate::diagnostics::invalid_index_diagnostic(*off, Some(len as u64)) });
            }
            Ok(IndexStep::Next(items[pos as usize]))
        }
        IndexSpec::Scalar(IntegerIndex(idx)) => {
            if *idx == 0 {
                return Ok(IndexStep::Next(match session.arena.get(expr) {
                    Some(athena_ir::TermNode::Collection { kind, .. }) => {
                        let kind = *kind;
                        let span = athena_ir::TermNode::default_span();
                        session.arena.push(athena_ir::TermNode::Collection { kind, elements: Vec::new() }, span)
                    }
                    Some(athena_ir::TermNode::Application { head, .. }) => {
                        let head = *head;
                        let span = athena_ir::TermNode::default_span();
                        session.arena.push(athena_ir::TermNode::Application { head, arguments: Vec::new() }, span)
                    }
                    _ => return Ok(IndexStep::Residual),
                }));
            }
            let pos = if *idx > 0 {
                (*idx - 1) as usize
            }
            else {
                let pos = len as i64 + *idx;
                if pos < 0 {
                    return Ok(IndexStep::Invalid {
                        echo: expr,
                        diagnostic: crate::diagnostics::invalid_index_diagnostic(*idx, Some(len as u64)),
                    });
                }
                pos as usize
            };
            match items.get(pos) {
                Some(item) => Ok(IndexStep::Next(*item)),
                None => Ok(IndexStep::Invalid { echo: expr, diagnostic: crate::diagnostics::invalid_index_diagnostic(*idx, Some(len as u64)) }),
            }
        }
        IndexSpec::Range { start, end, step } => {
            let Some(values) = expand_span_3(start.0, *step, end.0)
            else {
                return Ok(IndexStep::Residual);
            };
            let mut out = Vec::with_capacity(values.len());
            for v in values {
                match index_one(session, expr, &IndexSpec::Scalar(IntegerIndex(v)))? {
                    IndexStep::Next(item) => out.push(item),
                    IndexStep::Residual => return Ok(IndexStep::Residual),
                    IndexStep::Invalid { echo, diagnostic } => {
                        return Ok(IndexStep::Invalid { echo, diagnostic });
                    }
                }
            }
            Ok(IndexStep::Next(push_list(session, out)))
        }
        IndexSpec::Cartesian(axes) => {
            let mut cur = expr;
            for axis in axes {
                match index_one(session, cur, axis)? {
                    IndexStep::Next(next) => cur = next,
                    other => return Ok(other),
                }
            }
            Ok(IndexStep::Next(cur))
        }
        IndexSpec::DomainSpecific(_) => Ok(IndexStep::Residual),
    }
}

/// 对目标项执行完整轴序列。
pub(crate) fn evaluate_index_axes(session: &mut Session, mut cur: TermId, axes: &[IndexSpec]) -> Result<IndexOutcome> {
    if let [IndexSpec::All, rest @ ..] = axes {
        if !rest.is_empty() {
            if let Some(athena_ir::TermNode::Collection { elements: rows, .. }) = session.arena.get(cur) {
                let rows = rows.clone();
                let mut out = Vec::with_capacity(rows.len());
                for row in rows {
                    let mut cell = row;
                    for axis in rest {
                        match index_one(session, cell, axis)? {
                            IndexStep::Next(next) => cell = next,
                            IndexStep::Residual => return Ok(IndexOutcome::Term(cur)),
                            IndexStep::Invalid { echo, diagnostic } => {
                                return Ok(IndexOutcome::Invalid { echo, diagnostic });
                            }
                        }
                    }
                    out.push(cell);
                }
                return Ok(IndexOutcome::Term(push_list(session, out)));
            }
        }
    }

    for axis in axes {
        match index_one(session, cur, axis)? {
            IndexStep::Next(next) => cur = next,
            IndexStep::Residual => return Ok(IndexOutcome::Term(cur)),
            IndexStep::Invalid { echo, diagnostic } => {
                return Ok(IndexOutcome::Invalid { echo, diagnostic });
            }
        }
    }
    Ok(IndexOutcome::Term(cur))
}
