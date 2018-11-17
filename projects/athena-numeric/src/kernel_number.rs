//! [`NumericValue`] 内核算术（Living `16`；唯一数值真相源）。

use athena_types::{Diagnostic, DiagnosticCode, Result};

use crate::{integer::Integer, number::NumericValue, rational::Rational, real::Real};

enum Lifted {
    Integer(Integer),
    Rational(Rational),
    Real(f64),
}

fn lift(n: &NumericValue) -> Result<Lifted> {
    match n {
        NumericValue::Integer(i) => Ok(Lifted::Integer(i.clone())),
        NumericValue::Rational(r) => Ok(Lifted::Rational(r.clone())),
        NumericValue::Real(Real::Machine(x)) => Ok(Lifted::Real(*x)),
        NumericValue::Real(Real::BigFloat(b)) => b.to_f64_approximate().map(Lifted::Real).ok_or_else(|| {
            Diagnostic::new(DiagnosticCode::PromotionFailed).detail("domain", "numeric").detail("operation", "kernel_lift")
        }),
        _ => Err(Diagnostic::new(DiagnosticCode::UnsupportedOperation)
            .detail("domain", "numeric")
            .detail("operation", "kernel_lift")),
    }
}

fn unlift_exact_int(n: Integer) -> NumericValue {
    NumericValue::integer(n)
}

fn unlift_exact_rat(r: Rational) -> NumericValue {
    NumericValue::from_rational_normalized(r)
}

fn unlift_real(x: f64) -> NumericValue {
    NumericValue::machine(x)
}

/// 加法（含 promotion）。
pub fn add(a: NumericValue, b: NumericValue) -> Result<NumericValue> {
    match (lift(&a)?, lift(&b)?) {
        (Lifted::Integer(x), Lifted::Integer(y)) => Ok(unlift_exact_int(x.add(&y))),
        (Lifted::Rational(x), Lifted::Rational(y)) => Ok(unlift_exact_rat(x.add(&y))),
        (Lifted::Integer(x), Lifted::Rational(y)) => Ok(unlift_exact_rat(Rational::from_integer(x).add(&y))),
        (Lifted::Rational(x), Lifted::Integer(y)) => Ok(unlift_exact_rat(x.add(&Rational::from_integer(y)))),
        (x, y) => Ok(unlift_real(to_f64(&x)? + to_f64(&y)?)),
    }
}

/// 乘法（含 promotion）。
pub fn mul(a: NumericValue, b: NumericValue) -> Result<NumericValue> {
    match (lift(&a)?, lift(&b)?) {
        (Lifted::Integer(x), Lifted::Integer(y)) => Ok(unlift_exact_int(x.mul(&y))),
        (Lifted::Rational(x), Lifted::Rational(y)) => Ok(unlift_exact_rat(x.mul(&y))),
        (Lifted::Integer(x), Lifted::Rational(y)) => Ok(unlift_exact_rat(Rational::from_integer(x).mul(&y))),
        (Lifted::Rational(x), Lifted::Integer(y)) => Ok(unlift_exact_rat(x.mul(&Rational::from_integer(y)))),
        (x, y) => Ok(unlift_real(to_f64(&x)? * to_f64(&y)?)),
    }
}

/// 除法。
pub fn div(a: NumericValue, b: NumericValue) -> Result<NumericValue> {
    if b.is_zero() {
        return Err(Diagnostic::new(DiagnosticCode::DivideByZero));
    }
    match (lift(&a)?, lift(&b)?) {
        (Lifted::Integer(x), Lifted::Integer(y)) => Ok(unlift_exact_rat(Rational::try_new(x, y)?)),
        (Lifted::Rational(x), Lifted::Rational(y)) => Ok(unlift_exact_rat(x.try_div(&y)?)),
        (Lifted::Integer(x), Lifted::Rational(y)) => Ok(unlift_exact_rat(Rational::from_integer(x).try_div(&y)?)),
        (Lifted::Rational(x), Lifted::Integer(y)) => Ok(unlift_exact_rat(x.try_div(&Rational::from_integer(y))?)),
        (x, y) => Ok(unlift_real(to_f64(&x)? / to_f64(&y)?)),
    }
}

/// 幂。
pub fn pow(base: &NumericValue, exp: &NumericValue) -> Result<NumericValue> {
    if exp.is_zero() {
        return Ok(NumericValue::small_int(1));
    }
    if exp.is_one() {
        return Ok(base.clone());
    }
    if exp.is_neg_one() {
        return div(NumericValue::small_int(1), base.clone());
    }
    match (lift(base)?, lift(exp)?) {
        (Lifted::Integer(n), Lifted::Integer(e)) if !e.is_negative() => {
            Ok(unlift_exact_int(n.pow(&e).map_err(|_| Diagnostic::new(DiagnosticCode::ExponentOutOfRange))?))
        }
        (Lifted::Rational(r), Lifted::Integer(e)) if !e.is_negative() => {
            let u = e.to_u64().ok_or_else(|| Diagnostic::new(DiagnosticCode::ExponentOutOfRange))?;
            Ok(unlift_exact_rat(r.pow_u32(u as u32)?))
        }
        (Lifted::Integer(_) | Lifted::Rational(_), Lifted::Integer(e)) => {
            if let Some(i) = e.to_i64() {
                if i < 0 {
                    let v = pow(base, &NumericValue::small_int(-i))?;
                    return div(NumericValue::small_int(1), v);
                }
                return pow(base, &NumericValue::small_int(i));
            }
            Err(Diagnostic::new(DiagnosticCode::UnsupportedOperation))
        }
        (Lifted::Integer(_) | Lifted::Rational(_), Lifted::Rational(e)) => {
            if let Some(i) = e.is_integer().then(|| e.numerator().to_i64()).flatten() {
                return pow(base, &NumericValue::small_int(i));
            }
            Err(Diagnostic::new(DiagnosticCode::UnsupportedOperation))
        }
        (Lifted::Real(b), Lifted::Integer(e)) => {
            let ef = e.try_to_f64_exact().ok_or_else(|| Diagnostic::new(DiagnosticCode::ExponentOutOfRange))?;
            Ok(unlift_real(b.powf(ef)))
        }
        (Lifted::Real(b), Lifted::Real(e)) => Ok(unlift_real(b.powf(e))),
        _ => Err(Diagnostic::new(DiagnosticCode::UnsupportedOperation)),
    }
}

/// 非负精确整数阶乘。
pub fn factorial(n: &NumericValue) -> Result<NumericValue> {
    let i = match lift(n)? {
        Lifted::Integer(v) if v.is_negative() => return Err(Diagnostic::new(DiagnosticCode::DomainError)),
        Lifted::Integer(v) => v,
        Lifted::Rational(r) if r.is_integer() && r.numerator().is_non_negative() => r.numerator(),
        _ => return Err(Diagnostic::new(DiagnosticCode::TypeMismatch)),
    };
    let v = i.to_i64().ok_or_else(|| Diagnostic::new(DiagnosticCode::DomainError))?;
    if v > 10_000 {
        return Err(Diagnostic::new(DiagnosticCode::DomainError));
    }
    let mut acc = Integer::one();
    let mut k = 2i64;
    while k <= v {
        acc = acc.mul(&Integer::from_i64(k));
        k += 1;
    }
    Ok(unlift_exact_int(acc))
}

/// 平方根。
pub fn sqrt(n: &NumericValue) -> Result<Option<NumericValue>> {
    Ok(match lift(n)? {
        Lifted::Real(x) if x >= 0.0 => Some(unlift_real(x.sqrt())),
        Lifted::Real(_) => None,
        Lifted::Integer(v) if v.is_negative() => None,
        Lifted::Integer(v) => {
            let root = v.int_sqrt();
            if root.mul(&root) == v { Some(unlift_exact_int(root)) } else { None }
        }
        Lifted::Rational(r) if r.is_non_negative() => {
            let num = r.numerator().int_sqrt();
            let den = r.denominator().int_sqrt();
            if num.mul(&num) == r.numerator() && den.mul(&den) == r.denominator() {
                Some(unlift_exact_rat(Rational::try_new(num, den)?))
            }
            else {
                None
            }
        }
        Lifted::Rational(_) => None,
    })
}

/// 绝对值。
pub fn abs(n: NumericValue) -> NumericValue {
    match lift(&n) {
        Ok(Lifted::Integer(x)) => unlift_exact_int(x.abs()),
        Ok(Lifted::Rational(r)) => unlift_exact_rat(r.abs()),
        Ok(Lifted::Real(x)) => unlift_real(x.abs()),
        Err(_) => n,
    }
}

/// 取负。
pub fn neg(n: NumericValue) -> NumericValue {
    match lift(&n) {
        Ok(Lifted::Integer(x)) => unlift_exact_int(x.neg()),
        Ok(Lifted::Rational(r)) => unlift_exact_rat(r.neg()),
        Ok(Lifted::Real(x)) => unlift_real(-x),
        Err(_) => n,
    }
}

/// 比较（定义时）。
pub fn compare(a: &NumericValue, b: &NumericValue) -> Option<std::cmp::Ordering> {
    match (lift(a).ok()?, lift(b).ok()?) {
        (Lifted::Integer(x), Lifted::Integer(y)) => Some(x.cmp(&y)),
        (Lifted::Rational(x), Lifted::Rational(y)) => Some(x.cmp_numeric(&y)),
        (Lifted::Integer(x), Lifted::Rational(y)) => Some(Rational::from_integer(x).cmp_numeric(&y)),
        (Lifted::Rational(x), Lifted::Integer(y)) => Some(x.cmp_numeric(&Rational::from_integer(y))),
        (Lifted::Real(x), Lifted::Real(y)) => x.partial_cmp(&y),
        (Lifted::Integer(x), Lifted::Real(y)) => x.to_f64_approximate()?.partial_cmp(&y),
        (Lifted::Rational(x), Lifted::Real(y)) => x.to_f64_approximate()?.partial_cmp(&y),
        (Lifted::Real(x), Lifted::Integer(y)) => x.partial_cmp(&y.to_f64_approximate()?),
        (Lifted::Real(x), Lifted::Rational(y)) => x.partial_cmp(&y.to_f64_approximate()?),
    }
}

/// 有损 `f64`（宿主桥）。
pub fn to_f64_lossy(n: &NumericValue) -> Option<f64> {
    match lift(n).ok()? {
        Lifted::Integer(i) => i.to_f64_approximate(),
        Lifted::Rational(r) => r.to_f64_approximate(),
        Lifted::Real(x) => Some(x),
    }
}

fn to_f64(v: &Lifted) -> Result<f64> {
    match v {
        Lifted::Integer(n) => n.try_to_f64_exact().ok_or_else(|| Diagnostic::new(DiagnosticCode::PromotionFailed)),
        Lifted::Rational(r) => r.try_to_f64_exact().ok_or_else(|| Diagnostic::new(DiagnosticCode::PromotionFailed)),
        Lifted::Real(x) => Ok(*x),
    }
}
