//! `domains` 分组：域算法与多项式 parity fixture。

use athena_engine::{
    domains::polynomial::{
        CoefficientDomain, JitParityOutcome, MonomialOrder, PolynomialBuilder, RingTable, mul_with_jit_parity,
    },
    plot::{SampleDomain, SamplingPolicy, sample_1d},
    runtime::{
        Session,
        values::arena::{push_application_named, push_int, push_symbol_name},
    },
};
use athena_numeric::Number;
use athena_types::SymbolId;

fn square_of_x(session: &mut Session) -> athena_types::TermId {
    let x = push_symbol_name(session, "x");
    let two = push_int(session, 2);
    push_application_named(session, "Power", vec![x, two])
}

use crate::{
    fixture::{BenchGroup, Fixture, FixtureMeta, Suite},
    validate::{DeterminacyKind, ExactnessKind, ValidationSummary},
};

struct Sample1dFixture;

impl Fixture for Sample1dFixture {
    fn meta(&self) -> FixtureMeta {
        FixtureMeta::basic("domains.sample_1d_square", BenchGroup::Domains, "samples_17", "sampled_curve")
    }

    fn validate(&self) -> Result<ValidationSummary, String> {
        let mut session = Session::new();
        let expr = square_of_x(&mut session);
        let curve = sample_1d(&mut session, expr, "x", SampleDomain::new(-1.0, 1.0), SamplingPolicy::samples(5))
            .map_err(|d| d.code.as_str().to_string())?;
        if curve.points.len() != 5 {
            return Err(format!("expected 5 points, got {}", curve.points.len()));
        }
        if (curve.points[0].y - 1.0).abs() > 1e-9 {
            return Err("sample_1d endpoint mismatch".into());
        }
        Ok(ValidationSummary::passed(ExactnessKind::Machine, DeterminacyKind::Deterministic, "sample_1d x^2 on [-1,1]"))
    }

    fn run_once(&self) {
        let mut session = Session::new();
        let expr = square_of_x(&mut session);
        let _ = sample_1d(&mut session, expr, "x", SampleDomain::new(-1.0, 1.0), SamplingPolicy::samples(17));
    }
}

struct PolynomialMulParityFixture;

impl Fixture for PolynomialMulParityFixture {
    fn meta(&self) -> FixtureMeta {
        FixtureMeta::basic("domains.polynomial_mul_parity", BenchGroup::Domains, "univariate_z", "polynomial")
    }

    fn validate(&self) -> Result<ValidationSummary, String> {
        let mut rings = RingTable::new();
        let ring = rings
            .intern(CoefficientDomain::Integer, vec![SymbolId(0)], MonomialOrder::Lex)
            .map_err(|d| d.code.as_str().to_string())?;
        let mut b1 = PolynomialBuilder::new(ring);
        b1.push_term(Number::small_int(3), vec![1]).map_err(|d| d.code.as_str().to_string())?;
        let lhs = b1.build(&rings).map_err(|d| d.code.as_str().to_string())?;
        let mut b2 = PolynomialBuilder::new(ring);
        b2.push_term(Number::small_int(2), vec![1]).map_err(|d| d.code.as_str().to_string())?;
        let rhs = b2.build(&rings).map_err(|d| d.code.as_str().to_string())?;
        let (prod, parity) = mul_with_jit_parity(lhs, rhs, &rings).map_err(|d| d.code.as_str().to_string())?;
        if prod.terms().len() != 1 {
            return Err("expected single term product".into());
        }
        if !matches!(parity, JitParityOutcome::EagerOnly | JitParityOutcome::JitUnavailable) {
            return Err(format!("unexpected parity outcome: {parity:?}"));
        }
        Ok(ValidationSummary::passed(ExactnessKind::Exact, DeterminacyKind::Deterministic, "polynomial mul eager parity gate"))
    }

    fn run_once(&self) {
        let _ = self.validate();
    }
}

pub(super) fn register(suite: &mut Suite) {
    suite.register(Box::new(Sample1dFixture));
    suite.register(Box::new(PolynomialMulParityFixture));
}
