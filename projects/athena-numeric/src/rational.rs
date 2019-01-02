//! Exact rational wrapper (pure Rust; numerator / denominator are [`Integer`]).

use athena_types::{Diagnostic, DiagnosticCode};
use std::cmp::Ordering;

use crate::integer::{Integer, Sign};

/// Exact rational (reduced, positive denominator; also [`ExactRational`]).
///
/// Does not implement [`Ord`]: field lexicographic order is not numeric order.
/// Use [`Self::cmp_numeric`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Rational {
    numer: Integer,
    denom: Integer,
}

/// 稳定别名（与 [`NumericValue`] 同义迁移期命名）。
pub type ExactRational = Rational;

impl Rational {
    /// Construct from an integer.
    pub fn from_integer(n: Integer) -> Self {
        Self { numer: n, denom: Integer::one() }
    }

    /// Numerator / denominator (auto-reduced; zero denominator fails).
    pub fn try_new(numer: Integer, denom: Integer) -> Result<Self, Diagnostic> {
        if denom.is_zero() {
            return Err(Diagnostic::new(DiagnosticCode::DivideByZero)
                .detail("domain", "numeric")
                .detail("operation", "rational_new"));
        }
        Ok(Self::normalize_pair(numer, denom))
    }

    /// Numerator / denominator (auto-reduced). Panics on zero denominator — prefer [`try_new`].
    pub fn new(numer: Integer, denom: Integer) -> Self {
        Self::try_new(numer, denom).expect("rational denominator must be non-zero")
    }

    fn normalize_pair(numer: Integer, denom: Integer) -> Self {
        if denom.is_zero() {
            return Self { numer: Integer::zero(), denom: Integer::one() };
        }
        let g = numer.abs().gcd(&denom.abs());
        let mut n = if g.is_one() { numer } else { numer.div(&g).expect("gcd") };
        let mut d = if g.is_one() { denom } else { denom.div(&g).expect("gcd") };
        if d.is_negative() {
            n = n.neg();
            d = d.neg();
        }
        if d.is_one() { Self { numer: n, denom: Integer::one() } } else { Self { numer: n, denom: d } }
    }

    /// Explicit normalize (positive denominator, reduced).
    pub fn normalize(self) -> Self {
        Self::normalize_pair(self.numer, self.denom)
    }

    /// Zero.
    pub fn zero() -> Self {
        Self { numer: Integer::zero(), denom: Integer::one() }
    }

    /// One.
    pub fn one() -> Self {
        Self { numer: Integer::one(), denom: Integer::one() }
    }

    /// Numerator.
    pub fn numerator(&self) -> Integer {
        if self.denom.is_one() && !self.numer.is_zero() {
            self.numer.clone()
        }
        else if self.numer.is_zero() {
            Integer::zero()
        }
        else {
            self.numer.clone()
        }
    }

    /// Denominator (always positive; 1 for integers).
    pub fn denominator(&self) -> Integer {
        if self.numer.is_zero() { Integer::one() } else { self.denom.clone() }
    }

    /// Whether zero.
    pub fn is_zero(&self) -> bool {
        self.numer.is_zero()
    }

    /// Whether negative.
    pub fn is_negative(&self) -> bool {
        self.numer.is_negative()
    }

    /// Whether non-negative (zero counts as non-negative).
    pub fn is_non_negative(&self) -> bool {
        !self.numer.is_negative()
    }

    /// Whether an integer (denominator is 1).
    pub fn is_integer(&self) -> bool {
        self.denom.is_one()
    }

    /// Sign.
    pub fn sign(&self) -> Sign {
        self.numer.sign()
    }

    /// Numeric comparison with cross-cancellation before comparing `a*d` and `c*b`.
    pub fn cmp_numeric(&self, other: &Self) -> Ordering {
        if self == other {
            return Ordering::Equal;
        }
        let mut a = self.numer.clone();
        let mut b = self.denom.clone();
        let mut c = other.numer.clone();
        let mut d = other.denom.clone();
        let g1 = a.abs().gcd(&c.abs());
        if !g1.is_one() {
            a = a.div(&g1).expect("gcd");
            c = c.div(&g1).expect("gcd");
        }
        let g2 = b.abs().gcd(&d.abs());
        if !g2.is_one() {
            b = b.div(&g2).expect("gcd");
            d = d.div(&g2).expect("gcd");
        }
        a.mul(&d).cmp(&c.mul(&b))
    }

    /// Absolute value.
    pub fn abs(&self) -> Self {
        Self { numer: self.numer.abs(), denom: self.denom.clone() }
    }

    /// Negation.
    pub fn neg(&self) -> Self {
        Self { numer: self.numer.neg(), denom: self.denom.clone() }
    }

    /// Addition (cross-cancel `gcd(b,d)` before combining).
    pub fn add(&self, rhs: &Self) -> Self {
        let mut b = self.denom.clone();
        let mut d = rhs.denom.clone();
        let g = b.abs().gcd(&d.abs());
        if !g.is_one() {
            b = b.div(&g).expect("gcd");
            d = d.div(&g).expect("gcd");
        }
        let n = self.numer.mul(&d).add(&rhs.numer.mul(&b));
        let denom = self.denom.mul(&d);
        Self::normalize_pair(n, denom)
    }

    /// Subtraction.
    pub fn sub(&self, rhs: &Self) -> Self {
        self.add(&rhs.neg())
    }

    /// Multiplication (cross-cancel before product).
    pub fn mul(&self, rhs: &Self) -> Self {
        let (n, d) = cross_cancel_mul(self.numer.clone(), self.denom.clone(), rhs.numer.clone(), rhs.denom.clone());
        Self::normalize_pair(n, d)
    }

    /// Division (cross-cancel then multiply `a/b * d/c`).
    pub fn try_div(&self, rhs: &Self) -> Result<Self, Diagnostic> {
        if rhs.is_zero() {
            return Err(Diagnostic::new(DiagnosticCode::DivideByZero)
                .detail("domain", "numeric")
                .detail("operation", "rational_div"));
        }
        let (n, d) = cross_cancel_mul(self.numer.clone(), self.denom.clone(), rhs.denom.clone(), rhs.numer.clone());
        Ok(Self::normalize_pair(n, d))
    }

    /// Non-negative integer power.
    pub fn pow_u32(&self, exp: u32) -> Result<Self, Diagnostic> {
        if exp == 0 {
            return Ok(Self::one());
        }
        let n = self.numer.pow_u32(exp).map_err(|_| Diagnostic::new(DiagnosticCode::ExponentOutOfRange))?;
        let d = self.denom.pow_u32(exp).map_err(|_| Diagnostic::new(DiagnosticCode::ExponentOutOfRange))?;
        Ok(Self::normalize_pair(n, d))
    }

    /// Exact binary64 conversion when fully representable.
    pub fn try_to_f64_exact(&self) -> Option<f64> {
        if self.is_integer() {
            return self.numerator().try_to_f64_exact();
        }
        let d = self.denominator();
        if !d.is_power_of_two() {
            return None;
        }
        let nf = self.numerator().try_to_f64_exact()?;
        let df = d.try_to_f64_exact()?;
        if df == 0.0 {
            return None;
        }
        let q = nf / df;
        if !q.is_finite() {
            return None;
        }
        if nf.to_bits() == (q * df).to_bits() { Some(q) } else { None }
    }

    /// Explicit approximate `f64`.
    pub fn to_f64_approximate(&self) -> Option<f64> {
        let nf = self.numerator().to_f64_approximate()?;
        let df = self.denominator().to_f64_approximate()?;
        if df == 0.0 {
            return None;
        }
        let q = nf / df;
        if q.is_finite() { Some(q) } else { None }
    }

    /// Alias of [`try_to_f64_exact`].
    pub fn to_f64_exact_machine(&self) -> Option<f64> {
        self.try_to_f64_exact()
    }

    /// `numer/denom` decimal payload for host text rendering.
    pub fn to_wire_string(&self) -> String {
        if self.denom.is_one() {
            self.numer.to_decimal_string()
        }
        else {
            format!("{}/{}", self.numer.to_decimal_string(), self.denom.to_decimal_string())
        }
    }
}

/// Cross-cancel before multiplying `a/b * c/d`.
fn cross_cancel_mul(a: Integer, b: Integer, c: Integer, d: Integer) -> (Integer, Integer) {
    let mut a = a;
    let mut b = b;
    let mut c = c;
    let mut d = d;
    let g1 = a.abs().gcd(&d.abs());
    if !g1.is_one() {
        a = a.div(&g1).expect("gcd");
        d = d.div(&g1).expect("gcd");
    }
    let g2 = c.abs().gcd(&b.abs());
    if !g2.is_one() {
        c = c.div(&g2).expect("gcd");
        b = b.div(&g2).expect("gcd");
    }
    (a.mul(&c), b.mul(&d))
}
