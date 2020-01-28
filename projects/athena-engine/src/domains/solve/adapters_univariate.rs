//! 一元多项式根 → [`SolutionSet`] 适配（经因式分解，仅一次因子出根）。

use athena_numeric::{Number, div as num_div, neg as num_neg};
use athena_types::{Diagnostic, DiagnosticCode};

use crate::domains::polynomial::{
    Polynomial, PolynomialFactorLimits, PolynomialFactorization, PolynomialFactorizationCompleteness, RingTable, factor_univariate,
};

use super::{
    binding::{BindingMap, BoundSymbol},
    coverage::CoverageStatus,
    domain::SolveDomain,
    frontier::{ResumeKind, ResumeToken},
    map_coverage::coverage_from_factorization,
    solution::{BranchStatus, MultiplicityInfo, SolutionBranch, SolutionSet},
    value_table::{BindingValue, BindingValueTable},
};
use crate::runtime::values::numeric_clone::clone_number;

/// 一元根适配结果。
#[derive(Debug, PartialEq)]
pub struct UnivariateAdaptedSolution {
    /// 统一解集。
    pub solution: SolutionSet,
    /// 绑定值表。
    pub values: BindingValueTable,
    /// 底层因式分解完整性。
    pub factorization_completeness: PolynomialFactorizationCompleteness,
}

/// 由已有因式分解适配根集（goal = `PolynomialRootSet`）。
///
/// - 全部因子为一次且 `Complete` → 根集 `Complete`
/// - `Partial` / 高次余式 → 仅抽出已知一次根，覆盖 `CertifiedSubset`
/// - 常数非零 → 空解集 `Complete`（无根）
/// - 不得把未分裂高次因子冒充根
pub fn adapt_univariate_factorization(
    factorization: &PolynomialFactorization,
    unknown: BoundSymbol,
    domain: SolveDomain,
) -> Result<UnivariateAdaptedSolution, Diagnostic> {
    let completeness = factorization.completeness();
    let mut values = BindingValueTable::new();
    let mut branches = Vec::new();
    let mut all_extracted_linear = true;

    for factor in &factorization.factors {
        match linear_root_from_poly(&factor.base)? {
            Some(root) => {
                let term = values.intern(BindingValue::Number(root));
                let mut bindings = BindingMap::empty();
                bindings.insert(unknown, term);
                branches.push(SolutionBranch {
                    bindings,
                    conditions: super::constraint::ConstraintSet::empty_and(),
                    multiplicity: Some(MultiplicityInfo { algebraic: Some(factor.exponent), geometric: None }),
                    status: BranchStatus::Verified,
                });
            }
            None => {
                all_extracted_linear = false;
            }
        }
    }

    let coverage = match completeness {
        PolynomialFactorizationCompleteness::Complete if all_extracted_linear => CoverageStatus::Complete,
        PolynomialFactorizationCompleteness::Complete => CoverageStatus::Unsupported,
        other => coverage_from_factorization(other),
    };

    let frontier = match &coverage {
        CoverageStatus::ResourceLimited { frontier } => Some(frontier.clone()),
        _ if matches!(completeness, PolynomialFactorizationCompleteness::ResourceLimited) => {
            Some(ResumeToken::empty(ResumeKind::UnivariateFactor))
        }
        _ => None,
    };

    Ok(UnivariateAdaptedSolution {
        solution: SolutionSet { variables: vec![unknown], branches, coverage, domain, proof: None, residual: None, frontier },
        values,
        factorization_completeness: completeness,
    })
}

/// 因式分解后适配一元根。
pub fn solve_univariate_polynomial_roots(
    polynomial: Polynomial,
    rings: &RingTable,
    unknown: BoundSymbol,
    domain: SolveDomain,
    limits: PolynomialFactorLimits,
) -> Result<UnivariateAdaptedSolution, Diagnostic> {
    let factorization = factor_univariate(polynomial, rings, limits)?;
    adapt_univariate_factorization(&factorization, unknown, domain)
}

/// 一次多项式 `a x + b` 的根 `-b/a`；非一次返回 `Ok(None)`。
fn linear_root_from_poly(poly: &Polynomial) -> Result<Option<Number>, Diagnostic> {
    let mut a: Option<Number> = None;
    let mut b: Option<Number> = None;
    for term in poly.terms() {
        let exps = term.exponents();
        if exps.len() != 1 {
            return Err(diag("expected_univariate"));
        }
        match exps[0] {
            1 => a = Some(clone_number(term.coefficient())),
            0 => b = Some(clone_number(term.coefficient())),
            _ => return Ok(None),
        }
    }
    let Some(a) = a
    else {
        return Ok(None);
    };
    let b = b.unwrap_or_else(|| Number::small_int(0));
    let neg_b = num_neg(b);
    match num_div(neg_b, a) {
        Ok(root) => Ok(Some(root)),
        Err(e) => Err(e.detail("domain", "solve").detail("operation", "linear_root")),
    }
}

fn diag(reason: &str) -> Diagnostic {
    Diagnostic::new(DiagnosticCode::TypeMismatch).detail("domain", "solve").detail("operation", "adapt_univariate").detail("reason", reason)
}
