//! `numeric` 分组种子 fixture。

use athena_numeric::{
    DefaultPromotion, Integer, NumericDomain, NumericValue, Promotion, PromotionPolicy, Rational,
};

use crate::fixture::{BenchGroup, Fixture, FixtureMeta, Suite};
use crate::validate::{DeterminacyKind, ExactnessKind, ValidationSummary};

struct IntegerGcdFixture;

impl Fixture for IntegerGcdFixture {
    fn meta(&self) -> FixtureMeta {
        FixtureMeta {
            id: "numeric.integer_gcd",
            group: BenchGroup::Numeric,
            scale: "i64_pair",
            domain: "exact_integer",
        }
    }

    fn validate(&self) -> Result<ValidationSummary, String> {
        let g = Integer::from_i64(48).gcd(&Integer::from_i64(18));
        if g.to_decimal_string() != "6" {
            return Err(format!("gcd expected 6, got {}", g.to_decimal_string()));
        }
        Ok(ValidationSummary::passed(
            ExactnessKind::Exact,
            DeterminacyKind::Deterministic,
            "gcd(48,18)=6",
        ))
    }

    fn run_once(&self) {
        let _ = Integer::from_i64(123456789).gcd(&Integer::from_i64(987654321));
    }
}

struct RationalNormalizeFixture;

impl Fixture for RationalNormalizeFixture {
    fn meta(&self) -> FixtureMeta {
        FixtureMeta {
            id: "numeric.rational_normalize",
            group: BenchGroup::Numeric,
            scale: "small_rational",
            domain: "exact_rational",
        }
    }

    fn validate(&self) -> Result<ValidationSummary, String> {
        let r = Rational::new(Integer::from_i64(-2), Integer::from_i64(-4)).normalize();
        if r.numerator().to_decimal_string() != "1" || r.denominator().to_decimal_string() != "2" {
            return Err(format!(
                "normalize expected 1/2, got {}/{}",
                r.numerator().to_decimal_string(),
                r.denominator().to_decimal_string()
            ));
        }
        let a = NumericValue::integer(Integer::from_i64(5));
        let promoted = DefaultPromotion::promote(a, &NumericDomain::Rational, &PromotionPolicy::default())
            .map_err(|d| d.code.as_str().to_string())?;
        if promoted.domain != NumericDomain::Rational {
            return Err("promotion to rational failed".into());
        }
        Ok(ValidationSummary::passed(
            ExactnessKind::Exact,
            DeterminacyKind::Deterministic,
            "rational normalize + integer→rational promotion",
        ))
    }

    fn run_once(&self) {
        let _ = Rational::new(Integer::from_i64(222), Integer::from_i64(888)).normalize();
    }
}

pub(super) fn register(suite: &mut Suite) {
    suite.register(Box::new(IntegerGcdFixture));
    suite.register(Box::new(RationalNormalizeFixture));
}
