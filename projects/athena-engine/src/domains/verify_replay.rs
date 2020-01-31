//! PlanIR `Verify` recalculation (Living `28` / `29` bootstrap).
//!
//! Re-runs calculus / polynomial providers and compares against the claimed
//! `DomainResult`. Other domains keep a typed-presence gate until they gain
//! independent verifiers.
//!
//! **Does not** write AdmissionGate / SemanticCore. Certificate↔proposition
//! matching remains in [`crate::reasoning::mgraph::EvidenceVerifier`].

use athena_types::{Diagnostic, DiagnosticCode};

use crate::{
    domains::{
        calculus::{CalculusRequest, CalculusResult, CalculusValue, execute_calculus},
        dispatch::{DomainRequest, DomainResult},
        polynomial::{PolynomialRequest, PolynomialResult, execute_polynomial_with_rings},
    },
    runtime::session::Session,
};

/// Request snapshot retained across `CallDomainProvider` for Verify recompute.
#[derive(Debug, Clone)]
pub enum VerifySnapshot {
    /// Clone of a calculus request.
    Calculus(CalculusRequest),
    /// Clone of a polynomial request.
    Polynomial(PolynomialRequest),
    /// Domains without independent recompute yet.
    PresenceOnly,
}

impl VerifySnapshot {
    /// Capture a verify snapshot before the provider consumes the request.
    pub fn from_request(request: &DomainRequest) -> Self {
        match request {
            DomainRequest::Calculus(req) => Self::Calculus(req.clone()),
            DomainRequest::Polynomial(req) => Self::Polynomial(req.clone()),
            _ => Self::PresenceOnly,
        }
    }
}

/// Recompute and compare claimed provider output (PlanIR Verify body).
pub fn verify_recompute_domain_result(session: &mut Session, snapshot: &VerifySnapshot, claimed: &DomainResult) -> Result<(), Diagnostic> {
    match snapshot {
        VerifySnapshot::Calculus(req) => {
            let DomainResult::Calculus(claimed_calc) = claimed
            else {
                return Err(verify_err("calculus_result_kind_mismatch"));
            };
            let replay = execute_calculus(session, req.clone());
            assert_calculus_match(session, &replay, claimed_calc)
        }
        VerifySnapshot::Polynomial(req) => {
            let DomainResult::Polynomial(claimed_poly) = claimed
            else {
                return Err(verify_err("polynomial_result_kind_mismatch"));
            };
            // Always recompute via rings path (independent of M-Graph cache admit).
            let replay = execute_polynomial_with_rings(req.clone(), &session.rings, &session.polynomial_objects);
            assert_polynomial_match(session, &replay, claimed_poly)
        }
        VerifySnapshot::PresenceOnly => match claimed {
            DomainResult::Calculus(_)
            | DomainResult::NumberTheory(_)
            | DomainResult::Polynomial(_)
            | DomainResult::GroupTheory(_)
            | DomainResult::FieldTheory(_)
            | DomainResult::GaloisTheory(_)
            | DomainResult::GraphTheory(_)
            | DomainResult::LinearAlgebra(_)
            | DomainResult::Optimization(_) => Ok(()),
        },
    }
}

fn assert_calculus_match(
    session: &Session,
    replay: &CalculusResult<CalculusValue>,
    claimed: &CalculusResult<CalculusValue>,
) -> Result<(), Diagnostic> {
    match (replay, claimed) {
        (CalculusResult::Exact { value: rv, conditions: rc }, CalculusResult::Exact { value: cv, conditions: cc }) => {
            if rc != cc {
                return Err(verify_err("calculus_conditions_mismatch"));
            }
            if !calculus_values_match(session, rv, cv) {
                return Err(verify_err("calculus_recompute_mismatch"));
            }
            Ok(())
        }
        (CalculusResult::Unevaluated { .. }, CalculusResult::Unevaluated { .. }) => Ok(()),
        (CalculusResult::Conditional { value: rv, conditions: rc }, CalculusResult::Conditional { value: cv, conditions: cc }) => {
            if rc != cc {
                return Err(verify_err("calculus_conditions_mismatch"));
            }
            if !calculus_values_match(session, rv, cv) {
                return Err(verify_err("calculus_recompute_mismatch"));
            }
            Ok(())
        }
        _ => Err(verify_err("calculus_result_shape_mismatch")),
    }
}

fn calculus_values_match(session: &Session, a: &CalculusValue, b: &CalculusValue) -> bool {
    match (a, b) {
        (CalculusValue::Expression(x), CalculusValue::Expression(y)) => session.arena.structural_eq(*x, *y),
        (CalculusValue::Series(x), CalculusValue::Series(y)) => {
            if x == y {
                return true;
            }
            match (session.series_objects.get(*x), session.series_objects.get(*y)) {
                (Some(sx), Some(sy)) => sx == sy,
                _ => false,
            }
        }
        _ => a == b,
    }
}

fn assert_polynomial_match(
    session: &Session,
    replay: &PolynomialResult,
    claimed: &PolynomialResult,
) -> Result<(), Diagnostic> {
    match (replay, claimed) {
        (PolynomialResult::Exact { value: rv }, PolynomialResult::Exact { value: cv }) => {
            if rv != cv {
                return Err(verify_err("polynomial_recompute_mismatch"));
            }
            verify_claimed_groebner_basis(session, cv)
        }
        (PolynomialResult::Unevaluated { .. }, PolynomialResult::Unevaluated { .. }) => Ok(()),
        _ => Err(verify_err("polynomial_result_shape_mismatch")),
    }
}

fn verify_claimed_groebner_basis(session: &Session, value: &crate::domains::polynomial::PolynomialDomainValue) -> Result<(), Diagnostic> {
    use crate::domains::polynomial::{PolynomialDomainValue, verify_groebner_basis};
    let PolynomialDomainValue::GroebnerBasis(v) = value
    else {
        return Ok(());
    };
    if !v.is_exact_witness() {
        return Ok(());
    }
    let report = verify_groebner_basis(&v.basis, &session.rings).map_err(|_| verify_err("groebner_independent_verify_failed"))?;
    if report.all_s_pairs_reduce_to_zero {
        Ok(())
    }
    else {
        Err(verify_err("groebner_basis_not_complete"))
    }
}

fn verify_err(reason: &'static str) -> Diagnostic {
    Diagnostic::new(DiagnosticCode::UnsupportedOperation).detail("domain", "plan_exec").detail("reason", reason)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domains::calculus::DerivativeOrder;
    use athena_types::{AssumptionSet, SymbolId, TermId};

    #[test]
    fn calculus_forged_exact_term_fails_recompute() {
        let mut session = Session::new();
        let snapshot = VerifySnapshot::Calculus(CalculusRequest::Derivative {
            expression: TermId(0),
            variable: SymbolId(0),
            order: DerivativeOrder::First,
            assumptions: AssumptionSet::empty(),
        });
        let forged =
            DomainResult::Calculus(CalculusResult::Exact { value: CalculusValue::Expression(TermId(999_999)), conditions: Vec::new() });
        let err = verify_recompute_domain_result(&mut session, &snapshot, &forged).expect_err("forge");
        assert_eq!(err.details.get("reason").map(|v| v.to_string()).as_deref(), Some("calculus_recompute_mismatch"));
    }

    #[test]
    fn polynomial_forged_exact_fails_recompute() {
        use crate::domains::polynomial::{
            CoefficientDomain, MonomialOrder, PolynomialBuilder, PolynomialDomainValue, PolynomialRequest, PolynomialResult,
        };

        let mut session = Session::new();
        let ring = session.rings.intern(CoefficientDomain::Integer, vec![SymbolId(0)], MonomialOrder::Lex).expect("ring");
        let poly = PolynomialBuilder::new(ring).build(&session.rings).expect("zero");
        let poly_ref = session.polynomial_objects.intern(poly, &session.rings);
        let snapshot = VerifySnapshot::Polynomial(PolynomialRequest::Normalize { polynomial: poly_ref });
        let forged = DomainResult::Polynomial(PolynomialResult::Exact { value: PolynomialDomainValue::Placeholder });
        let err = verify_recompute_domain_result(&mut session, &snapshot, &forged).expect_err("forge");
        assert_eq!(err.details.get("reason").map(|v| v.to_string()).as_deref(), Some("polynomial_recompute_mismatch"));
    }
}
