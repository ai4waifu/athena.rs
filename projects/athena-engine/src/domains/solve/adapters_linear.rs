//! 线性系统 → [`SolutionSet`] 适配。

use athena_types::{Diagnostic, DiagnosticCode};

use crate::domains::linear_algebra::{
    ExactSolveResult, MachineSolveResult, MatrixEntry, MatrixValue, SolveDisposition, solve_exact, solve_machine,
};

use super::{
    binding::{BindingMap, BoundSymbol},
    certificate::ResidualCertificate,
    coverage::CoverageStatus,
    domain::SolveDomain,
    map_coverage::{coverage_from_exact_disposition, coverage_from_machine_disposition},
    solution::{BranchStatus, SolutionBranch, SolutionSet},
    value_table::{BindingValue, BindingValueTable},
};

/// 线性适配结果：解集 + 绑定值表 + 原始 disposition。
#[derive(Debug, PartialEq)]
pub struct LinearAdaptedSolution {
    /// 统一解集。
    pub solution: SolutionSet,
    /// [`TermId`] 句柄对应的标量。
    pub values: BindingValueTable,
    /// 原始分类。
    pub disposition: SolveDisposition,
}

/// 精确线性系统 → [`SolutionSet`]（goal = `LinearSystemSolve`）。
pub fn adapt_exact_linear_solve(
    result: ExactSolveResult,
    unknowns: Vec<BoundSymbol>,
    domain: SolveDomain,
) -> Result<LinearAdaptedSolution, Diagnostic> {
    let disposition = result.disposition.clone();
    let coverage = coverage_from_exact_disposition(&disposition);
    let mut values = BindingValueTable::new();
    let branches = match &disposition {
        SolveDisposition::Unique => {
            let particular = result.particular.ok_or_else(|| diag("unique_missing_particular"))?;
            vec![branch_from_column(&particular, &unknowns, &mut values, BranchStatus::Verified)?]
        }
        SolveDisposition::Inconsistent => Vec::new(),
        SolveDisposition::Infinite { .. } => {
            let particular = result.particular.ok_or_else(|| diag("infinite_missing_particular"))?;
            // 无零空间基：仅特解分支，coverage 为 CertifiedSubset。
            vec![branch_from_column(&particular, &unknowns, &mut values, BranchStatus::Conditional)?]
        }
        SolveDisposition::Singular | SolveDisposition::ResourceLimited => Vec::new(),
    };
    let frontier = coverage_frontier(&coverage);
    Ok(LinearAdaptedSolution {
        solution: SolutionSet { variables: unknowns, branches, coverage, domain, proof: None, residual: None, frontier },
        values,
        disposition,
    })
}

/// 调用 [`solve_exact`] 并适配。
pub fn solve_linear_system_exact(
    a: &MatrixValue,
    b: &MatrixValue,
    unknowns: Vec<BoundSymbol>,
    domain: SolveDomain,
) -> Result<LinearAdaptedSolution, Diagnostic> {
    if unknowns.len() as u64 != a.shape().cols {
        return Err(diag("unknowns_cols_mismatch"));
    }
    adapt_exact_linear_solve(solve_exact(a, b)?, unknowns, domain)
}

/// 机器线性系统 → [`SolutionSet`]（局部覆盖，不进 exact union-find）。
pub fn adapt_machine_linear_solve(
    result: MachineSolveResult,
    unknowns: Vec<BoundSymbol>,
    domain: SolveDomain,
) -> Result<LinearAdaptedSolution, Diagnostic> {
    let disposition = result.disposition.clone();
    let coverage = coverage_from_machine_disposition(&disposition, result.guarantee);
    let mut values = BindingValueTable::new();
    let branches = match &disposition {
        SolveDisposition::Unique => {
            let sol = result.solution.ok_or_else(|| diag("unique_missing_solution"))?;
            vec![branch_from_column(&sol, &unknowns, &mut values, BranchStatus::Verified)?]
        }
        _ => Vec::new(),
    };
    let residual = result.witness.as_ref().map(|w| ResidualCertificate {
        residual: values.intern(BindingValue::MachineF64(w.residual_inf)),
        residual_is_zero: w.residual_inf.abs() <= w.pivot_threshold,
        condition_note: Some(format!("numerical_rank={}", w.numerical_rank)),
    });
    let frontier = coverage_frontier(&coverage);
    Ok(LinearAdaptedSolution {
        solution: SolutionSet { variables: unknowns, branches, coverage, domain, proof: None, residual, frontier },
        values,
        disposition,
    })
}

/// 调用 [`solve_machine`] 并适配。
pub fn solve_linear_system_machine(
    a: &MatrixValue,
    b: &MatrixValue,
    unknowns: Vec<BoundSymbol>,
    domain: SolveDomain,
    pivot_threshold: f64,
) -> Result<LinearAdaptedSolution, Diagnostic> {
    if unknowns.len() as u64 != a.shape().cols {
        return Err(diag("unknowns_cols_mismatch"));
    }
    adapt_machine_linear_solve(solve_machine(a, b, pivot_threshold)?, unknowns, domain)
}

fn coverage_frontier(coverage: &CoverageStatus) -> Option<super::frontier::ResumeToken> {
    match coverage {
        CoverageStatus::ResourceLimited { frontier } => Some(frontier.clone()),
        _ => None,
    }
}

fn branch_from_column(
    column: &MatrixValue,
    unknowns: &[BoundSymbol],
    values: &mut BindingValueTable,
    status: BranchStatus,
) -> Result<SolutionBranch, Diagnostic> {
    if column.shape().cols != 1 || column.shape().rows != unknowns.len() as u64 {
        return Err(diag("particular_shape_mismatch"));
    }
    let mut bindings = BindingMap::empty();
    for (i, unknown) in unknowns.iter().enumerate() {
        let entry = column.get(i as u64, 0)?;
        let value = binding_value_from_entry(entry)?;
        let term = values.intern(value);
        bindings.insert(*unknown, term);
    }
    Ok(SolutionBranch { bindings, conditions: super::constraint::ConstraintSet::empty_and(), multiplicity: None, status })
}

fn binding_value_from_entry(entry: MatrixEntry) -> Result<BindingValue, Diagnostic> {
    match entry {
        MatrixEntry::Integer(n) => Ok(BindingValue::Rational(athena_numeric::Rational::from_integer(n))),
        MatrixEntry::Rational(r) => Ok(BindingValue::Rational(r)),
        MatrixEntry::MachineF64(x) => Ok(BindingValue::MachineF64(x)),
    }
}

fn diag(reason: &str) -> Diagnostic {
    Diagnostic::new(DiagnosticCode::TypeMismatch)
        .detail("domain", "solve")
        .detail("operation", "adapt_linear")
        .detail("reason", reason)
}
