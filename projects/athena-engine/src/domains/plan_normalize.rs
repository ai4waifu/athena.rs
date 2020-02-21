//! `DomainPlan` `Normalize` coercion (Living `28`).
//!
//! Validates DomainObject handles and calculus `TermId`s, and rewrites polynomial
//! refs onto canonical interned identities before `CallDomainProvider`.

use athena_types::{Diagnostic, DiagnosticCode, TermId};

use crate::{
    domains::{
        calculus::{CalculusRequest, LimitApproach},
        dispatch::DomainRequest,
        field::FieldRequest,
        galois::GaloisRequest,
        group::GroupRequest,
        linear_algebra::LinearAlgebraRequest,
        polynomial::{PolynomialRef, PolynomialRequest, canonicalize_polynomial},
    },
    runtime::session::Session,
};

/// Outcome of a `DomainPlan` `Normalize` step.
#[derive(Debug)]
pub struct NormalizeOutcome {
    /// Request ready for the provider (handles validated / coerced).
    pub request: DomainRequest,
    /// True when at least one polynomial handle was rewritten to a canonical intern.
    pub coerced: bool,
}

/// Normalize / coerce a [`DomainRequest`] against the live Session stores.
pub fn normalize_domain_request(session: &mut Session, request: DomainRequest) -> Result<NormalizeOutcome, Diagnostic> {
    match request {
        DomainRequest::Polynomial(req) => normalize_polynomial(session, req),
        DomainRequest::LinearAlgebra(req) => {
            validate_linear_algebra(session, &req)?;
            Ok(NormalizeOutcome { request: DomainRequest::LinearAlgebra(req), coerced: false })
        }
        DomainRequest::Calculus(req) => {
            validate_calculus(session, &req)?;
            Ok(NormalizeOutcome { request: DomainRequest::Calculus(req), coerced: false })
        }
        DomainRequest::GroupTheory(req) => {
            validate_group(session, &req)?;
            Ok(NormalizeOutcome { request: DomainRequest::GroupTheory(req), coerced: false })
        }
        DomainRequest::FieldTheory(req) => {
            validate_field(session, &req)?;
            Ok(NormalizeOutcome { request: DomainRequest::FieldTheory(req), coerced: false })
        }
        DomainRequest::GaloisTheory(req) => {
            validate_galois(session, &req)?;
            Ok(NormalizeOutcome { request: DomainRequest::GaloisTheory(req), coerced: false })
        }
        other => Ok(NormalizeOutcome { request: other, coerced: false }),
    }
}

fn normalize_polynomial(session: &mut Session, request: PolynomialRequest) -> Result<NormalizeOutcome, Diagnostic> {
    let mut coerced = false;
    let mut map = |session: &mut Session, r: PolynomialRef| -> Result<PolynomialRef, Diagnostic> {
        let (nr, changed) = coerce_polynomial_ref(session, r)?;
        coerced |= changed;
        Ok(nr)
    };
    let request = match request {
        PolynomialRequest::Normalize { polynomial } => {
            PolynomialRequest::Normalize { polynomial: map(session, polynomial)? }
        }
        PolynomialRequest::Add { lhs, rhs } => {
            PolynomialRequest::Add { lhs: map(session, lhs)?, rhs: map(session, rhs)? }
        }
        PolynomialRequest::Mul { lhs, rhs } => {
            PolynomialRequest::Mul { lhs: map(session, lhs)?, rhs: map(session, rhs)? }
        }
        PolynomialRequest::Div { dividend, divisor, policy } => PolynomialRequest::Div {
            dividend: map(session, dividend)?,
            divisor: map(session, divisor)?,
            policy,
        },
        PolynomialRequest::Gcd { lhs, rhs } => {
            PolynomialRequest::Gcd { lhs: map(session, lhs)?, rhs: map(session, rhs)? }
        }
        PolynomialRequest::Factor { polynomial, limits } => {
            PolynomialRequest::Factor { polynomial: map(session, polynomial)?, limits }
        }
        PolynomialRequest::Groebner { generators, limits } => PolynomialRequest::Groebner {
            generators: map_poly_refs(session, generators, &mut coerced)?,
            limits,
        },
        PolynomialRequest::GroebnerF4 { generators, limits } => PolynomialRequest::GroebnerF4 {
            generators: map_poly_refs(session, generators, &mut coerced)?,
            limits,
        },
        PolynomialRequest::Eliminate { generators, limits } => PolynomialRequest::Eliminate {
            generators: map_poly_refs(session, generators, &mut coerced)?,
            limits,
        },
        PolynomialRequest::ResumeGroebner {
            candidates,
            pending_pairs,
            pending_insertion,
            input_generators,
            prior_s_pair_steps,
            limits,
        } => PolynomialRequest::ResumeGroebner {
            candidates: map_poly_refs(session, candidates, &mut coerced)?,
            pending_pairs,
            pending_insertion: match pending_insertion {
                Some(r) => {
                    let (nr, changed) = coerce_polynomial_ref(session, r)?;
                    coerced |= changed;
                    Some(nr)
                }
                None => None,
            },
            input_generators,
            prior_s_pair_steps,
            limits,
        },
        PolynomialRequest::ResumeGroebnerF4 {
            candidates,
            pending_pairs,
            pending_insertion,
            input_generators,
            prior_s_pair_steps,
            candidate_sugars,
            pending_insertion_sugar,
            limits,
        } => PolynomialRequest::ResumeGroebnerF4 {
            candidates: map_poly_refs(session, candidates, &mut coerced)?,
            pending_pairs,
            pending_insertion: match pending_insertion {
                Some(r) => {
                    let (nr, changed) = coerce_polynomial_ref(session, r)?;
                    coerced |= changed;
                    Some(nr)
                }
                None => None,
            },
            input_generators,
            prior_s_pair_steps,
            candidate_sugars,
            pending_insertion_sugar,
            limits,
        },
        PolynomialRequest::ModularImage { polynomial, image_ring } => {
            PolynomialRequest::ModularImage { polynomial: map(session, polynomial)?, image_ring }
        }
        PolynomialRequest::ReconstructModular { image, target_ring } => {
            PolynomialRequest::ReconstructModular { image: map(session, image)?, target_ring }
        }
        PolynomialRequest::CrtCombineModular { images, integer_ring, target_ring } => PolynomialRequest::CrtCombineModular {
            images: map_poly_refs(session, images, &mut coerced)?,
            integer_ring,
            target_ring,
        },
    };
    Ok(NormalizeOutcome { request: DomainRequest::Polynomial(request), coerced })
}

fn map_poly_refs(session: &mut Session, refs: Vec<PolynomialRef>, coerced: &mut bool) -> Result<Vec<PolynomialRef>, Diagnostic> {
    let mut out = Vec::with_capacity(refs.len());
    for r in refs {
        let (nr, changed) = coerce_polynomial_ref(session, r)?;
        *coerced |= changed;
        out.push(nr);
    }
    Ok(out)
}

fn coerce_polynomial_ref(session: &mut Session, r: PolynomialRef) -> Result<(PolynomialRef, bool), Diagnostic> {
    let poly = session.polynomial_objects.resolve_owning(r).map_err(|_| {
        Diagnostic::new(DiagnosticCode::UnsupportedOperation)
            .detail("domain", "plan_normalize")
            .detail("reason", "missing_polynomial_ref")
            .arg("ref", r.0)
    })?;
    let canon = canonicalize_polynomial(poly, &session.rings).map_err(|e| {
        Diagnostic::new(DiagnosticCode::UnsupportedOperation)
            .detail("domain", "plan_normalize")
            .detail("reason", "polynomial_canonicalize_failed")
            .detail("inner", e.code.as_str())
    })?;
    let nr = session.polynomial_objects.intern(canon, &session.rings);
    Ok((nr, nr != r))
}

fn validate_linear_algebra(session: &Session, request: &LinearAlgebraRequest) -> Result<(), Diagnostic> {
    let check = |r: crate::domains::linear_algebra::MatrixRef| -> Result<(), Diagnostic> {
        if session.matrix_objects.get(r).is_some() {
            Ok(())
        } else {
            Err(Diagnostic::new(DiagnosticCode::UnsupportedOperation)
                .detail("domain", "plan_normalize")
                .detail("reason", "missing_matrix_ref")
                .arg("ref", r.0))
        }
    };
    match *request {
        LinearAlgebraRequest::Transpose { matrix }
        | LinearAlgebraRequest::Index { matrix, .. }
        | LinearAlgebraRequest::Rank { matrix }
        | LinearAlgebraRequest::Det { matrix }
        | LinearAlgebraRequest::Rref { matrix } => check(matrix),
        LinearAlgebraRequest::MatMul { lhs, rhs }
        | LinearAlgebraRequest::Hadamard { lhs, rhs }
        | LinearAlgebraRequest::Solve { a: lhs, b: rhs } => {
            check(lhs)?;
            check(rhs)
        }
    }
}

fn validate_calculus(session: &Session, request: &CalculusRequest) -> Result<(), Diagnostic> {
    let check_term = |id: TermId| -> Result<(), Diagnostic> {
        if session.arena.get(id).is_some() {
            Ok(())
        } else {
            Err(Diagnostic::new(DiagnosticCode::UnsupportedOperation)
                .detail("domain", "plan_normalize")
                .detail("reason", "missing_term_id")
                .arg("term", id.0))
        }
    };
    match request {
        CalculusRequest::Derivative { expression, .. }
        | CalculusRequest::Integral { expression, .. }
        | CalculusRequest::Asymptotic { expression, .. }
        | CalculusRequest::Gradient { expression, .. }
        | CalculusRequest::Hessian { expression, .. }
        | CalculusRequest::Transform { expression, .. } => check_term(*expression),
        CalculusRequest::Limit { expression, approach, .. } => {
            check_term(*expression)?;
            if let LimitApproach::Finite(point) = approach {
                check_term(*point)?;
            }
            Ok(())
        }
        CalculusRequest::DefiniteIntegral { expression, lower, upper, .. } => {
            check_term(*expression)?;
            check_term(*lower)?;
            check_term(*upper)
        }
        CalculusRequest::Series { expression, center, .. } | CalculusRequest::Laurent { expression, center, .. } => {
            check_term(*expression)?;
            check_term(*center)
        }
        CalculusRequest::Residue { expression, point, .. } => {
            check_term(*expression)?;
            check_term(*point)
        }
        CalculusRequest::Jacobian { expressions, .. } => {
            for id in expressions {
                check_term(*id)?;
            }
            Ok(())
        }
        CalculusRequest::Divergence { components, .. } | CalculusRequest::Curl { components, .. } => {
            for id in components {
                check_term(*id)?;
            }
            Ok(())
        }
        CalculusRequest::SolveOde { equation, initial, .. } => {
            check_term(*equation)?;
            if let Some((x0, y0)) = initial {
                check_term(*x0)?;
                check_term(*y0)?;
            }
            Ok(())
        }
    }
}

fn validate_group(session: &Session, request: &GroupRequest) -> Result<(), Diagnostic> {
    match request {
        GroupRequest::Order { group } | GroupRequest::IsAbelian { group } => {
            if session.groups.group_record(*group).is_ok() {
                Ok(())
            } else {
                Err(Diagnostic::new(DiagnosticCode::UnsupportedOperation)
                    .detail("domain", "plan_normalize")
                    .detail("reason", "missing_group_id")
                    .arg("group", group.0))
            }
        }
        GroupRequest::IsNormalSubgroup { subgroup }
        | GroupRequest::QuotientGroup { subgroup }
        | GroupRequest::ProjectQuotient { subgroup, .. } => {
            if session.groups.subgroup_record(*subgroup).is_ok() {
                Ok(())
            } else {
                Err(Diagnostic::new(DiagnosticCode::UnsupportedOperation)
                    .detail("domain", "plan_normalize")
                    .detail("reason", "missing_subgroup_id")
                    .arg("subgroup", subgroup.0))
            }
        }
        GroupRequest::Multiply { .. }
        | GroupRequest::Inverse { .. }
        | GroupRequest::ApplyHomomorphism { .. }
        | GroupRequest::Cyclic { .. }
        | GroupRequest::PermutationGroup { .. }
        | GroupRequest::SubgroupFromGenerators { .. }
        | GroupRequest::HomomorphismFromGeneratorImages { .. } => Ok(()),
    }
}

fn validate_field(session: &Session, request: &FieldRequest) -> Result<(), Diagnostic> {
    match request {
        FieldRequest::Lookup { field } => {
            if session.rings.field_table().field_record(*field).is_ok() {
                Ok(())
            } else {
                Err(Diagnostic::new(DiagnosticCode::UnsupportedOperation)
                    .detail("domain", "plan_normalize")
                    .detail("reason", "missing_field_id")
                    .arg("field", field.0))
            }
        }
        FieldRequest::PrimeField { .. }
        | FieldRequest::Rationals
        | FieldRequest::Add { .. }
        | FieldRequest::Mul { .. }
        | FieldRequest::Inverse { .. } => Ok(()),
    }
}

fn validate_galois(session: &Session, request: &GaloisRequest) -> Result<(), Diagnostic> {
    match request {
        GaloisRequest::IsExtensionSeparable { extension }
        | GaloisRequest::IsExtensionNormal { extension }
        | GaloisRequest::IsGalois { extension }
        | GaloisRequest::GaloisGroupOfExtension { extension }
        | GaloisRequest::FixedField { extension, .. } => {
            if session.rings.field_table().extension_record(*extension).is_some() {
                Ok(())
            } else {
                Err(Diagnostic::new(DiagnosticCode::UnsupportedOperation)
                    .detail("domain", "plan_normalize")
                    .detail("reason", "missing_extension_id")
                    .arg("extension", extension.0))
            }
        }
        GaloisRequest::IsPolynomialSeparable { .. } | GaloisRequest::GaloisGroupOfPolynomial { .. } => Ok(()),
    }
}
