//! 中立 `IndexSpec` 求值（Reference 与 `ExecutionHost` 共用）。

use athena_ir::{ApplicationHead, SemanticOperator};
use athena_numeric::{Integer, Rational};
use athena_types::{Diagnostic, IndexSpec, IntegerIndex, IntegerOffset, Result, TermId};

use super::{evaluate_apply_head_terms, expand_span_3, matrix_entry_to_term_session, matrix_to_nested_list_session};
use crate::{
    domains::linear_algebra::{ElementParentKind, MatrixEntry, MatrixValue},
    execution::number_of,
    runtime::{session::Session, values::arena::push_list, values::numeric_clone::clone_rational},
};

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

/// 矩阵 DomainObject 上的 `StoreIndex` 结果。
#[derive(Debug)]
pub(crate) enum MatrixStoreOutcome {
    /// 新 intern 的矩阵运行时值。
    Value(athena_types::ValueId),
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
        // Handled by `evaluate_index_axes` (needs full target shape). Fallback: scalar.
        IndexSpec::LinearColumnMajor(IntegerIndex(idx)) => {
            index_one(session, expr, &IndexSpec::Scalar(IntegerIndex(*idx)))
        }
        IndexSpec::ColumnMajorFlatten => Ok(IndexStep::Residual),
        IndexSpec::DomainSpecific(_) => Ok(IndexStep::Residual),
    }
}

/// 对目标项执行完整轴序列。
pub(crate) fn evaluate_index_axes(session: &mut Session, mut cur: TermId, axes: &[IndexSpec]) -> Result<IndexOutcome> {
    // MATLAB `f(k)` is parsed as Part/Index. When Own is `Function[var, body]`, apply instead.
    if is_function_term(session, cur) {
        if let Some(args) = call_args_from_axes(session, axes) {
            let term = evaluate_apply_head_terms(session, cur, args)?;
            return Ok(IndexOutcome::Term(term));
        }
    }

    // Free / non-indexable head (`speye(2)` before any Own): keep call args, do not strip to `speye`.
    if !is_indexable_target(session, cur) {
        if let Some(args) = call_args_from_axes(session, axes) {
            let term = evaluate_apply_head_terms(session, cur, args)?;
            return Ok(IndexOutcome::Term(term));
        }
    }

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

    // MATLAB `A(:)`: column-major flatten once the runtime target shape is known.
    if let [IndexSpec::ColumnMajorFlatten] = axes {
        return Ok(IndexOutcome::Term(flatten_column_major(session, cur)));
    }

    // MATLAB `A(k)`: rewrite column-major linear index once the runtime target shape is known.
    let axes_owned;
    let axes = if let [IndexSpec::LinearColumnMajor(IntegerIndex(k))] = axes {
        axes_owned = rewrite_linear_column_major(session, cur, *k);
        axes_owned.as_slice()
    }
    else {
        axes
    };

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

/// 对矩阵 DomainObject 执行 `IndexSpec` 轴序列（MATLAB 列优先线性 / 二维标量）。
pub(crate) fn evaluate_index_axes_matrix(session: &mut Session, matrix: &MatrixValue, axes: &[IndexSpec]) -> Result<IndexOutcome> {
    let echo = matrix_to_nested_list_session(session, matrix).unwrap_or_else(|_| session.builder().null(Default::default()));
    match matrix_axes_to_cell(matrix, axes) {
        MatrixAxes::Cell(row, col) => match matrix_entry_to_term_session(session, matrix, row, col) {
            Ok(term) => Ok(IndexOutcome::Term(term)),
            Err(diagnostic) => Ok(IndexOutcome::Invalid { echo, diagnostic }),
        },
        MatrixAxes::Flatten => flatten_matrix_column_major(session, matrix),
        MatrixAxes::Invalid(index) => Ok(IndexOutcome::Invalid {
            echo,
            diagnostic: crate::diagnostics::invalid_index_diagnostic(index, matrix.shape().element_count().ok().map(|n| n as u64)),
        }),
        MatrixAxes::Unsupported => Ok(IndexOutcome::Invalid {
            echo,
            diagnostic: Diagnostic::new(athena_types::DiagnosticCode::UnsupportedOperation)
                .detail("component", "evaluate_index_axes_matrix")
                .detail("reason", "unsupported_matrix_index_axes"),
        }),
        MatrixAxes::Grow { .. } => Ok(IndexOutcome::Invalid {
            echo,
            diagnostic: Diagnostic::new(athena_types::DiagnosticCode::UnsupportedOperation)
                .detail("component", "evaluate_index_axes_matrix")
                .detail("reason", "matrix_index_grow_on_read"),
        }),
    }
}

/// 对矩阵 DomainObject 写入后 intern 新句柄（写时复制）。
///
/// 支持行/列向量线性扩容、`end+k`，以及二维单元格越界扩形（零填充）。
pub(crate) fn store_index_axes_matrix(
    session: &mut Session,
    mut matrix: MatrixValue,
    axes: &[IndexSpec],
    value: TermId,
) -> Result<MatrixStoreOutcome> {
    let echo = matrix_to_nested_list_session(session, &matrix).unwrap_or_else(|_| session.builder().null(Default::default()));
    let (row, col) = match matrix_axes_to_store_target(&matrix, axes, true) {
        MatrixAxes::Cell(row, col) => (row, col),
        MatrixAxes::Grow { row, col, rows, cols } => {
            let grown = if cols == 1 && matrix.shape().cols > 1 {
                column_major_grow_vector(&matrix, rows)
            } else {
                matrix.resize_owned(rows, cols)
            };
            match grown {
                Ok(m) => matrix = m,
                Err(diagnostic) => return Ok(MatrixStoreOutcome::Invalid { echo, diagnostic }),
            }
            (row, col)
        }
        MatrixAxes::Invalid(index) => {
            return Ok(MatrixStoreOutcome::Invalid {
                echo,
                diagnostic: crate::diagnostics::invalid_index_diagnostic(index, matrix.shape().element_count().ok().map(|n| n as u64)),
            });
        }
        MatrixAxes::Flatten | MatrixAxes::Unsupported => {
            return Ok(MatrixStoreOutcome::Invalid {
                echo,
                diagnostic: Diagnostic::new(athena_types::DiagnosticCode::UnsupportedOperation)
                    .detail("component", "store_index_axes_matrix")
                    .detail("reason", "unsupported_matrix_store_axes"),
            });
        }
    };
    let entry = match term_to_matrix_entry(session, value, matrix.parent().element) {
        Ok(entry) => entry,
        Err(diagnostic) => return Ok(MatrixStoreOutcome::Invalid { echo, diagnostic }),
    };
    if let Err(diagnostic) = matrix.set_owned(row, col, entry) {
        return Ok(MatrixStoreOutcome::Invalid { echo, diagnostic });
    }
    let stored = session.matrix_objects.intern(matrix);
    Ok(MatrixStoreOutcome::Value(session.insert_matrix_value(stored)))
}

enum MatrixAxes {
    Cell(u64, u64),
    /// 写入前需扩容到 `(rows, cols)`，再写 `(row, col)`。
    Grow {
        row: u64,
        col: u64,
        rows: u64,
        cols: u64,
    },
    Flatten,
    Invalid(i64),
    Unsupported,
}

fn matrix_axes_to_cell(matrix: &MatrixValue, axes: &[IndexSpec]) -> MatrixAxes {
    // 读路径不允许扩容：越界为正下标时返回 Invalid。
    matrix_axes_to_store_target(matrix, axes, false)
}

/// 解析矩阵读写目标。`allow_grow` 为 true 时，越界正下标可扩容。
fn matrix_axes_to_store_target(matrix: &MatrixValue, axes: &[IndexSpec], allow_grow: bool) -> MatrixAxes {
    let nrows = matrix.shape().rows;
    let ncols = matrix.shape().cols;
    match axes {
        [IndexSpec::LinearColumnMajor(IntegerIndex(k))] => linear_column_major_target(nrows, ncols, *k, allow_grow),
        // 单轴 Scalar 与线性同合同（部分宿主未改写为 LinearColumnMajor）。
        [IndexSpec::Scalar(IntegerIndex(k))] => linear_column_major_target(nrows, ncols, *k, allow_grow),
        [IndexSpec::EndRelative(IntegerOffset(off))] => {
            let numel = nrows.saturating_mul(ncols) as i64;
            linear_column_major_target(nrows, ncols, numel + *off, allow_grow)
        }
        [IndexSpec::Scalar(IntegerIndex(r)), IndexSpec::Scalar(IntegerIndex(c))] => one_based_cell_target(*r, *c, nrows, ncols, allow_grow),
        [IndexSpec::ColumnMajorFlatten] => MatrixAxes::Flatten,
        _ => MatrixAxes::Unsupported,
    }
}

fn linear_column_major_target(nrows: u64, ncols: u64, k: i64, allow_grow: bool) -> MatrixAxes {
    if k < 1 {
        return MatrixAxes::Invalid(k);
    }
    let n = (k as u64) - 1;
    let len = nrows.saturating_mul(ncols);
    if n < len && nrows > 0 {
        let col = n / nrows;
        let row = n % nrows;
        return MatrixAxes::Cell(row, col);
    }
    if !allow_grow {
        return MatrixAxes::Invalid(k);
    }
    // 行向量保形扩列；列向量保形扩行；一般矩阵线性越界 → 列向量（MATLAB 合同）。
    if nrows == 1 {
        MatrixAxes::Grow {
            row: 0,
            col: n,
            rows: 1,
            cols: n + 1,
        }
    } else {
        MatrixAxes::Grow {
            row: n,
            col: 0,
            rows: n + 1,
            cols: 1,
        }
    }
}

fn one_based_cell_target(row_1: i64, col_1: i64, nrows: u64, ncols: u64, allow_grow: bool) -> MatrixAxes {
    if row_1 < 1 || col_1 < 1 {
        return MatrixAxes::Invalid(row_1);
    }
    let row = (row_1 as u64) - 1;
    let col = (col_1 as u64) - 1;
    if row < nrows && col < ncols {
        return MatrixAxes::Cell(row, col);
    }
    if !allow_grow {
        return MatrixAxes::Invalid(row_1);
    }
    MatrixAxes::Grow {
        row,
        col,
        rows: nrows.max(row + 1),
        cols: ncols.max(col + 1),
    }
}

/// 列优先展平后扩成 `new_len×1` 列向量（一般矩阵线性越界写入）。
fn column_major_grow_vector(matrix: &MatrixValue, new_len: u64) -> Result<MatrixValue> {
    use crate::domains::linear_algebra::{MatrixShape, StorageOrder};
    let (nrows, ncols) = (matrix.shape().rows, matrix.shape().cols);
    let mut out = MatrixValue::zeros(matrix.parent(), MatrixShape::new(new_len, 1), StorageOrder::RowMajor)?;
    let mut i = 0u64;
    for c in 0..ncols {
        for r in 0..nrows {
            if i >= new_len {
                break;
            }
            out.set_owned(i, 0, matrix.get(r, c)?)?;
            i += 1;
        }
    }
    Ok(out)
}

fn flatten_matrix_column_major(session: &mut Session, matrix: &MatrixValue) -> Result<IndexOutcome> {
    let (nrows, ncols) = (matrix.shape().rows, matrix.shape().cols);
    let mut out = Vec::with_capacity((nrows.saturating_mul(ncols)) as usize);
    for c in 0..ncols {
        for r in 0..nrows {
            let cell = matrix_entry_to_term_session(session, matrix, r, c)?;
            out.push(push_list(session, vec![cell]));
        }
    }
    Ok(IndexOutcome::Term(push_list(session, out)))
}

fn term_to_matrix_entry(session: &Session, term: TermId, kind: ElementParentKind) -> Result<MatrixEntry> {
    let Some(n) = number_of(session, term)
    else {
        return Err(Diagnostic::new(athena_types::DiagnosticCode::TypeMismatch).detail("reason", "store_value_not_number"));
    };
    match kind {
        ElementParentKind::Integers => {
            if let Some(i) = n.as_exact_integer() {
                Ok(MatrixEntry::Integer(Integer::from(i)))
            }
            else {
                Err(Diagnostic::new(athena_types::DiagnosticCode::TypeMismatch).detail("reason", "store_value_not_integer"))
            }
        }
        ElementParentKind::Rationals => {
            if let Some(i) = n.as_exact_integer() {
                Ok(MatrixEntry::Rational(Rational::from_integer(Integer::from(i))))
            }
            else if let Some(r) = n.as_rational() {
                Ok(MatrixEntry::Rational(clone_rational(r)))
            }
            else {
                Err(Diagnostic::new(athena_types::DiagnosticCode::TypeMismatch).detail("reason", "store_value_not_rational"))
            }
        }
        ElementParentKind::MachineReal => {
            if let Some(x) = n.as_machine_f64() {
                Ok(MatrixEntry::MachineF64(x))
            }
            else if let Some(i) = n.as_exact_integer() {
                Ok(MatrixEntry::MachineF64(i as f64))
            }
            else {
                Err(Diagnostic::new(athena_types::DiagnosticCode::TypeMismatch).detail("reason", "store_value_not_machine"))
            }
        }
    }
}

fn nested_matrix_shape(session: &Session, term: TermId) -> Option<(usize, usize)> {
    let rows = match session.arena.get(term)? {
        athena_ir::TermNode::Collection { elements, .. } => elements.clone(),
        _ => return None,
    };
    if rows.is_empty() {
        return None;
    }
    let first = match session.arena.get(rows[0])? {
        athena_ir::TermNode::Collection { elements, .. } => elements.clone(),
        _ => return None,
    };
    let ncols = first.len();
    for row in &rows {
        match session.arena.get(*row)? {
            athena_ir::TermNode::Collection { elements, .. } if elements.len() == ncols => {}
            _ => return None,
        }
    }
    Some((rows.len(), ncols))
}

fn rewrite_linear_column_major(session: &Session, target: TermId, k: i64) -> Vec<IndexSpec> {
    if k < 1 {
        return vec![IndexSpec::Scalar(IntegerIndex(k))];
    }
    if let Some((nrows, _ncols)) = nested_matrix_shape(session, target) {
        if nrows > 0 {
            let r = ((k - 1).rem_euclid(nrows as i64)) + 1;
            let c = ((k - 1) / nrows as i64) + 1;
            return vec![IndexSpec::Scalar(IntegerIndex(r)), IndexSpec::Scalar(IntegerIndex(c))];
        }
    }
    vec![IndexSpec::Scalar(IntegerIndex(k))]
}

fn is_function_term(session: &Session, term: TermId) -> bool {
    matches!(
        session.arena.get(term),
        Some(athena_ir::TermNode::Application { head: ApplicationHead::Semantic(SemanticOperator::Function), .. })
    )
}

fn is_indexable_target(session: &Session, term: TermId) -> bool {
    matches!(
        session.arena.get(term),
        Some(athena_ir::TermNode::Collection { .. } | athena_ir::TermNode::Application { .. })
    )
}

/// Rebuild call arguments from Index axes when the target is a call head, not a container.
fn call_args_from_axes(session: &mut Session, axes: &[IndexSpec]) -> Option<Vec<TermId>> {
    if axes.is_empty() {
        return None;
    }
    let mut args = Vec::with_capacity(axes.len());
    for axis in axes {
        match axis {
            IndexSpec::Scalar(IntegerIndex(k)) | IndexSpec::LinearColumnMajor(IntegerIndex(k)) => {
                args.push(session.builder().int(*k, Default::default()));
            }
            _ => return None,
        }
    }
    Some(args)
}

/// 1-based scalar / linear / `EndRelative` store into a collection.
/// Out-of-range positive indices grow the collection, padding with exact `0`.
pub(crate) fn store_index_axes(session: &mut Session, cur: TermId, axes: &[IndexSpec], value: TermId) -> Result<IndexOutcome> {
    let items = match session.arena.get(cur) {
        Some(athena_ir::TermNode::Collection { elements, .. }) => elements.clone(),
        _ => {
            return Ok(IndexOutcome::Invalid {
                echo: cur,
                diagnostic: crate::diagnostics::invalid_index_diagnostic(0, None)
                    .detail("reason", "store_index_target_not_collection"),
            });
        }
    };
    let len = items.len();
    match axes {
        [IndexSpec::Scalar(IntegerIndex(k))] | [IndexSpec::LinearColumnMajor(IntegerIndex(k))] => {
            store_flat_at_one_based(session, cur, &items, len, *k, value)
        }
        [IndexSpec::EndRelative(IntegerOffset(off))] => {
            // `end` → len, `end+1` → len+1 (grow/append).
            let one_based = len as i64 + *off;
            store_flat_at_one_based(session, cur, &items, len, one_based, value)
        }
        [IndexSpec::Scalar(IntegerIndex(r)), IndexSpec::Scalar(IntegerIndex(c))] => {
            store_nested_matrix_cell(session, cur, *r, *c, value, &items, len)
        }
        _ => Ok(IndexOutcome::Invalid {
            echo: cur,
            diagnostic: Diagnostic::new(athena_types::DiagnosticCode::UnsupportedOperation)
                .detail("component", "store_index_axes")
                .detail("reason", "unsupported_store_axes"),
        }),
    }
}

fn exact_zero(session: &mut Session) -> TermId {
    session.builder().int(0, Default::default())
}

fn store_flat_at_one_based(
    session: &mut Session,
    cur: TermId,
    items: &[TermId],
    len: usize,
    one_based: i64,
    value: TermId,
) -> Result<IndexOutcome> {
    if one_based == 0 {
        return Ok(IndexOutcome::Invalid {
            echo: cur,
            diagnostic: crate::diagnostics::invalid_index_diagnostic(one_based, Some(len as u64)),
        });
    }
    let pos = if one_based > 0 {
        (one_based - 1) as usize
    } else {
        let pos = len as i64 + one_based;
        if pos < 0 {
            return Ok(IndexOutcome::Invalid {
                echo: cur,
                diagnostic: crate::diagnostics::invalid_index_diagnostic(one_based, Some(len as u64)),
            });
        }
        pos as usize
    };
    let mut next = items.to_vec();
    if pos >= next.len() {
        let zero = exact_zero(session);
        next.resize(pos + 1, zero);
    }
    next[pos] = value;
    Ok(IndexOutcome::Term(push_list(session, next)))
}

fn store_nested_matrix_cell(
    session: &mut Session,
    cur: TermId,
    row_1based: i64,
    col_1based: i64,
    value: TermId,
    rows: &[TermId],
    nrows: usize,
) -> Result<IndexOutcome> {
    if row_1based <= 0 || col_1based <= 0 {
        return Ok(IndexOutcome::Invalid {
            echo: cur,
            diagnostic: crate::diagnostics::invalid_index_diagnostic(row_1based, Some(nrows as u64)),
        });
    }
    let ri = (row_1based - 1) as usize;
    let ci = (col_1based - 1) as usize;

    let current_ncols = rows
        .first()
        .and_then(|row| match session.arena.get(*row) {
            Some(athena_ir::TermNode::Collection { elements, .. }) => Some(elements.len()),
            _ => None,
        })
        .unwrap_or(0);
    let target_ncols = current_ncols.max(ci + 1);

    let mut new_rows = Vec::with_capacity(nrows.max(ri + 1));
    for r in 0..nrows.max(ri + 1) {
        let mut cols = if r < nrows {
            match session.arena.get(rows[r]) {
                Some(athena_ir::TermNode::Collection { elements, .. }) => elements.clone(),
                _ => {
                    return Ok(IndexOutcome::Invalid {
                        echo: cur,
                        diagnostic: Diagnostic::new(athena_types::DiagnosticCode::UnsupportedOperation)
                            .detail("component", "store_index_axes")
                            .detail("reason", "store_index_row_not_collection"),
                    });
                }
            }
        } else {
            Vec::new()
        };
        if cols.len() < target_ncols {
            let zero = exact_zero(session);
            cols.resize(target_ncols, zero);
        }
        if r == ri {
            cols[ci] = value;
        }
        new_rows.push(push_list(session, cols));
    }
    Ok(IndexOutcome::Term(push_list(session, new_rows)))
}

fn flatten_column_major(session: &mut Session, target: TermId) -> TermId {
    let Some((nrows, ncols)) = nested_matrix_shape(session, target)
    else {
        // Flat / non-matrix: All semantics — return the collection as-is via IndexSpec::All.
        return match index_one(session, target, &IndexSpec::All) {
            Ok(IndexStep::Next(t)) => t,
            _ => target,
        };
    };
    let rows = match session.arena.get(target) {
        Some(athena_ir::TermNode::Collection { elements, .. }) => elements.clone(),
        _ => return target,
    };
    let mut out = Vec::with_capacity(nrows * ncols);
    for c in 0..ncols {
        for r in 0..nrows {
            if let Some(athena_ir::TermNode::Collection { elements: cols, .. }) = session.arena.get(rows[r]) {
                if let Some(cell) = cols.get(c) {
                    // Column vector as nested 1-cell rows: [[e1],[e2],…]
                    out.push(push_list(session, vec![*cell]));
                }
            }
        }
    }
    push_list(session, out)
}
