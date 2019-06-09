//! 单变量多项式除法 · GCD · Resultant（ℤ / ℚ / 𝔽_p）。

use athena_numeric::{Integer, Number, add as num_add, div as num_div, mul as num_mul, neg as num_neg};
use athena_types::{Diagnostic, DiagnosticCode, Result, RingId};

use super::{
    builder::PolynomialBuilder,
    coefficient_kernel::CoefficientRing,
    object::Polynomial,
    ring::{CoefficientDomain, DivisionPolicy},
    ring_table::RingTable,
};
use crate::runtime::values::numeric_clone::{clone_integer, clone_number, clone_numbers, clone_rational, resize_numbers};

/// 单变量除法结果。
#[derive(Debug, PartialEq)]
pub struct UnivariateDivision {
    /// 商。
    pub quotient: Polynomial,
    /// 余式。
    pub remainder: Polynomial,
}

/// 单变量精确除法。
pub fn div_univariate(dividend: Polynomial, divisor: Polynomial, policy: DivisionPolicy, rings: &RingTable) -> Result<UnivariateDivision> {
    ensure_same_ring(&dividend, &divisor)?;
    let desc = rings.get(dividend.ring()).ok_or_else(|| ring_unknown(dividend.ring()))?;
    let var = detect_univariate_var(&dividend, desc.variable_count())?;
    let a = to_dense(&dividend, var, desc.variable_count())?;
    let b = to_dense(&divisor, var, desc.variable_count())?;
    let domain = rings.coefficient_domain_for_descriptor(desc).ok_or_else(|| ring_unknown(dividend.ring()))?;
    let (q, r) = match domain {
        CoefficientDomain::Rational | CoefficientDomain::FiniteField { .. } => {
            let coeff = rings.coefficient_kernel(dividend.ring())?;
            div_dense_field(&a, &b, &coeff, policy)?
        }
        CoefficientDomain::Integer => div_dense_integer(&a, &b, policy)?,
        _ => return Err(unsupported_domain()),
    };
    Ok(UnivariateDivision {
        quotient: from_dense(&q, var, desc.variable_count(), dividend.ring(), rings)?,
        remainder: from_dense(&r, var, desc.variable_count(), dividend.ring(), rings)?,
    })
}

/// 单变量 GCD（零多项式返回零；非零时返回 primitive 意义下规范代表）。
pub fn gcd_univariate(lhs: Polynomial, rhs: Polynomial, rings: &RingTable) -> Result<Polynomial> {
    ensure_same_ring(&lhs, &rhs)?;
    let desc = rings.get(lhs.ring()).ok_or_else(|| ring_unknown(lhs.ring()))?;
    let var = detect_univariate_var(&lhs, desc.variable_count())?;
    let a = to_dense(&lhs, var, desc.variable_count())?;
    let b = to_dense(&rhs, var, desc.variable_count())?;
    let domain = rings.coefficient_domain_for_descriptor(desc).ok_or_else(|| ring_unknown(lhs.ring()))?;
    let g = match domain {
        CoefficientDomain::Rational | CoefficientDomain::FiniteField { .. } => {
            let coeff = rings.coefficient_kernel(lhs.ring())?;
            gcd_dense_field(a, b, &coeff)?
        }
        CoefficientDomain::Integer => gcd_dense_integer(a, b)?,
        _ => return Err(unsupported_domain()),
    };
    from_dense(&g, var, desc.variable_count(), lhs.ring(), rings)
}

/// 单变量 Sylvester 结式（系数环中的标量）。
pub fn resultant_univariate(lhs: Polynomial, rhs: Polynomial, rings: &RingTable) -> Result<Number> {
    ensure_same_ring(&lhs, &rhs)?;
    let desc = rings.get(lhs.ring()).ok_or_else(|| ring_unknown(lhs.ring()))?;
    let var = detect_univariate_var(&lhs, desc.variable_count())?;
    let a = to_dense(&lhs, var, desc.variable_count())?;
    let b = to_dense(&rhs, var, desc.variable_count())?;
    let domain = rings.coefficient_domain_for_descriptor(desc).ok_or_else(|| ring_unknown(lhs.ring()))?;
    resultant_dense(&a, &b, domain, lhs.ring(), rings)
}

fn div_dense_field(a: &[Number], b: &[Number], coeff: &CoefficientRing<'_>, policy: DivisionPolicy) -> Result<(Vec<Number>, Vec<Number>)> {
    if is_zero_dense(b) {
        return Err(division_by_zero());
    }
    let mut rem = trim_dense(a);
    let mut b = trim_dense(b);
    if degree(&rem) < degree(&b) {
        return Ok((Vec::new(), rem));
    }
    let mut quot = {
        let mut __v = Vec::new();
        resize_numbers(&mut __v, degree(&rem) - degree(&b) + 1, &Number::integer(Integer::zero()));
        __v
    };
    while degree(&rem) >= degree(&b) && !is_zero_dense(&rem) {
        let d = degree(&rem) - degree(&b);
        let q_coeff = coeff.div(lc(&rem)?, lc(&b)?)?;
        quot[d] = coeff.add(clone_number(&quot[d]), clone_number(&q_coeff))?;
        rem = sub_scaled_monomial(&rem, &b, q_coeff, d, coeff)?;
    }
    if policy == DivisionPolicy::ExactOnly && !is_zero_dense(&rem) {
        return Err(Diagnostic::new(DiagnosticCode::PolynomialNonFieldDivision)
            .detail("domain", "polynomial")
            .detail("operation", "exact_division_failed"));
    }
    Ok((trim_dense(&quot), rem))
}

fn div_dense_integer(a: &[Number], b: &[Number], policy: DivisionPolicy) -> Result<(Vec<Number>, Vec<Number>)> {
    if is_zero_dense(b) {
        return Err(division_by_zero());
    }
    let (q_rat, r_rat) = div_dense_rational(a, b)?;
    match policy {
        DivisionPolicy::FieldDivision => {
            return Err(Diagnostic::new(DiagnosticCode::PolynomialNonFieldDivision)
                .detail("domain", "polynomial")
                .detail("operation", "field_division_in_integer_ring"));
        }
        DivisionPolicy::ExactOnly | DivisionPolicy::PromoteToRational => {
            if !is_zero_dense(&r_rat) {
                return Err(Diagnostic::new(DiagnosticCode::PolynomialNonFieldDivision)
                    .detail("domain", "polynomial")
                    .detail("operation", "exact_division_failed"));
            }
            ensure_integer_coeffs(&q_rat)?;
            Ok((q_rat, r_rat))
        }
        DivisionPolicy::PseudoDivision => {
            let (pq, pr) = pseudo_divide(a, b)?;
            ensure_integer_coeffs(&pq)?;
            ensure_integer_coeffs(&pr)?;
            Ok((pq, pr))
        }
    }
}

fn div_dense_rational(a: &[Number], b: &[Number]) -> Result<(Vec<Number>, Vec<Number>)> {
    if is_zero_dense(b) {
        return Err(division_by_zero());
    }
    let mut rem = trim_dense(a);
    let mut b = trim_dense(b);
    if degree(&rem) < degree(&b) {
        return Ok((Vec::new(), rem));
    }
    let mut quot = {
        let mut __v = Vec::new();
        resize_numbers(&mut __v, degree(&rem) - degree(&b) + 1, &Number::integer(Integer::zero()));
        __v
    };
    while degree(&rem) >= degree(&b) && !is_zero_dense(&rem) {
        let d = degree(&rem) - degree(&b);
        let q_coeff = num_div(lc(&rem)?, lc(&b)?)?;
        quot[d] = num_add(clone_number(&quot[d]), clone_number(&q_coeff))?;
        rem = sub_scaled_monomial_rational(&rem, &b, q_coeff, d)?;
    }
    Ok((trim_dense(&quot), rem))
}

fn pseudo_divide(a: &[Number], b: &[Number]) -> Result<(Vec<Number>, Vec<Number>)> {
    if is_zero_dense(b) {
        return Err(division_by_zero());
    }
    let mut rem = trim_dense(a);
    let b = trim_dense(b);
    if degree(&rem) < degree(&b) {
        return Ok((Vec::new(), rem));
    }
    let delta = degree(&rem) - degree(&b) + 1;
    let scale = num_pow(lc(&b)?, delta)?;
    let mut quot = {
        let mut __v = Vec::new();
        resize_numbers(&mut __v, delta, &Number::integer(Integer::zero()));
        __v
    };
    let mut rem = scale_dense(&rem, clone_number(&scale))?;
    while degree(&rem) >= degree(&b) && !is_zero_dense(&rem) {
        let d = degree(&rem) - degree(&b);
        let q_coeff = lc(&rem)?;
        quot[d] = num_add(clone_number(&quot[d]), clone_number(&q_coeff))?;
        rem = sub_scaled_monomial_rational(&rem, &b, q_coeff, d)?;
    }
    Ok((trim_dense(&quot), rem))
}

fn gcd_dense_field(a: Vec<Number>, b: Vec<Number>, coeff: &CoefficientRing<'_>) -> Result<Vec<Number>> {
    let mut a = trim_dense(&a);
    let mut b = trim_dense(&b);
    if is_zero_dense(&b) {
        return Ok(monic_dense(&a, coeff)?);
    }
    while !is_zero_dense(&b) {
        let (_, r) = div_dense_field(&a, &b, coeff, DivisionPolicy::FieldDivision)?;
        a = b;
        b = r;
    }
    monic_dense(&a, coeff)
}

fn gcd_dense_integer(a: Vec<Number>, b: Vec<Number>) -> Result<Vec<Number>> {
    let ca = content_dense(&a)?;
    let cb = content_dense(&b)?;
    let pp_a = primitive_part_dense(&a, &ca)?;
    let pp_b = primitive_part_dense(&b, &cb)?;
    let g_pp = gcd_dense_rational(pp_a, pp_b)?;
    let g_content = ca.gcd(&cb);
    scale_dense(&g_pp, Number::integer(g_content))
}

fn gcd_dense_rational(mut a: Vec<Number>, mut b: Vec<Number>) -> Result<Vec<Number>> {
    a = trim_dense(&a);
    b = trim_dense(&b);
    if is_zero_dense(&b) {
        return monic_dense_rational(&a);
    }
    while !is_zero_dense(&b) {
        let (_, r) = div_dense_rational(&a, &b)?;
        a = b;
        b = r;
    }
    monic_dense_rational(&a)
}

fn resultant_dense(a: &[Number], b: &[Number], domain: &CoefficientDomain, ring: RingId, rings: &RingTable) -> Result<Number> {
    let a = trim_dense(a);
    let b = trim_dense(b);
    if is_zero_dense(&a) || is_zero_dense(&b) {
        return Ok(Number::small_int(0));
    }
    let m = degree(&a);
    let n = degree(&b);
    if m == 0 {
        return Ok(num_pow(lc(&a)?, n)?);
    }
    if n == 0 {
        return Ok(num_pow(lc(&b)?, m)?);
    }
    let size = m + n;
    let mut mat = {
        let mut mat = Vec::with_capacity(size);
        for _ in 0..size {
            let mut __v = Vec::new();
            resize_numbers(&mut __v, size, &Number::integer(Integer::zero()));
            mat.push(__v);
        }
        mat
    };
    for row in 0..n {
        for (col, c) in a.iter().enumerate() {
            if row + col < size {
                mat[row][row + col] = clone_number(&c);
            }
        }
    }
    for row in 0..m {
        for (col, c) in b.iter().enumerate() {
            if row + col < size {
                mat[n + row][row + col] = clone_number(&c);
            }
        }
    }
    let det = det_matrix(&mat, domain, ring, rings)?;
    normalize_resultant_sign(m, n, det, domain, ring, rings)
}

/// Sylvester 行列式符号规范化：约定 `Res(f,g) = (-1)^(deg f · deg g) · det(S)`。
fn normalize_resultant_sign(
    deg_a: usize,
    deg_b: usize,
    det: Number,
    domain: &CoefficientDomain,
    ring: RingId,
    rings: &RingTable,
) -> Result<Number> {
    if (deg_a * deg_b) % 2 == 0 {
        return Ok(det);
    }
    match domain {
        CoefficientDomain::Rational | CoefficientDomain::Integer => Ok(num_neg(det)),
        CoefficientDomain::FiniteField { .. } => {
            let coeff = rings.coefficient_kernel(ring)?;
            coeff.neg(det)
        }
        _ => Err(unsupported_domain()),
    }
}

fn det_matrix(mat: &[Vec<Number>], domain: &CoefficientDomain, ring: RingId, rings: &RingTable) -> Result<Number> {
    let n = mat.len();
    if n == 0 {
        return Ok(Number::small_int(1));
    }
    match domain {
        CoefficientDomain::Rational | CoefficientDomain::Integer => det_rational(mat.iter().map(|row| clone_numbers(row)).collect()),
        CoefficientDomain::FiniteField { .. } => {
            let coeff = rings.coefficient_kernel(ring)?;
            det_field(mat.iter().map(|row| clone_numbers(row)).collect(), &coeff)
        }
        _ => Err(unsupported_domain()),
    }
}

fn det_rational(mut a: Vec<Vec<Number>>) -> Result<Number> {
    let n = a.len();
    let mut det = Number::small_int(1);
    for col in 0..n {
        let mut pivot = col;
        while pivot < n && a[pivot][col].is_zero() {
            pivot += 1;
        }
        if pivot == n {
            return Ok(Number::small_int(0));
        }
        if pivot != col {
            a.swap(col, pivot);
            det = num_neg(det);
        }
        let pivot_val = clone_number(&a[col][col]);
        det = num_mul(det, clone_number(&pivot_val))?;
        for row in (col + 1)..n {
            if a[row][col].is_zero() {
                continue;
            }
            let factor = num_div(clone_number(&a[row][col]), clone_number(&pivot_val))?;
            for k in col..n {
                let sub = num_mul(clone_number(&factor), clone_number(&a[col][k]))?;
                a[row][k] = num_add(clone_number(&a[row][k]), num_neg(sub))?;
            }
        }
    }
    Ok(det)
}

fn det_field(mut a: Vec<Vec<Number>>, coeff: &CoefficientRing<'_>) -> Result<Number> {
    let n = a.len();
    let mut det = Number::small_int(1);
    for col in 0..n {
        let mut pivot = col;
        while pivot < n && a[pivot][col].is_zero() {
            pivot += 1;
        }
        if pivot == n {
            return Ok(Number::small_int(0));
        }
        if pivot != col {
            a.swap(col, pivot);
            det = coeff.neg(det)?;
        }
        let pivot_val = clone_number(&a[col][col]);
        det = coeff.mul(det, clone_number(&pivot_val))?;
        for row in (col + 1)..n {
            if a[row][col].is_zero() {
                continue;
            }
            let factor = coeff.div(clone_number(&a[row][col]), clone_number(&pivot_val))?;
            for k in col..n {
                let sub = coeff.mul(clone_number(&factor), clone_number(&a[col][k]))?;
                a[row][k] = coeff.add(clone_number(&a[row][k]), coeff.neg(sub)?)?;
            }
        }
    }
    Ok(det)
}

fn detect_univariate_var(poly: &Polynomial, n: usize) -> Result<usize> {
    if n == 0 {
        return Err(Diagnostic::new(DiagnosticCode::PolynomialVariableMismatch)
            .detail("domain", "polynomial")
            .detail("operation", "univariate_no_variables"));
    }
    if n == 1 {
        return Ok(0);
    }
    if poly.terms().is_empty() {
        return Ok(0);
    }
    let mut active = None;
    for term in poly.terms() {
        if term.exponents().len() != n {
            return Err(exponent_mismatch());
        }
        for (i, &e) in term.exponents().iter().enumerate() {
            if e != 0 {
                match active {
                    None => active = Some(i),
                    Some(v) if v == i => {}
                    _ => {
                        return Err(Diagnostic::new(DiagnosticCode::PolynomialVariableMismatch)
                            .detail("domain", "polynomial")
                            .detail("operation", "univariate_multivariate"));
                    }
                }
            }
        }
    }
    active.ok_or_else(|| Diagnostic::new(DiagnosticCode::DomainError).detail("domain", "polynomial").detail("operation", "zero_polynomial"))
}

fn to_dense(poly: &Polynomial, var: usize, n: usize) -> Result<Vec<Number>> {
    let mut max = 0usize;
    for term in poly.terms() {
        if term.exponents().len() != n {
            return Err(exponent_mismatch());
        }
        for (i, &e) in term.exponents().iter().enumerate() {
            if i != var && e != 0 {
                return Err(Diagnostic::new(DiagnosticCode::PolynomialVariableMismatch)
                    .detail("domain", "polynomial")
                    .detail("operation", "univariate_multivariate"));
            }
        }
        max = max.max(term.exponents()[var] as usize);
    }
    let mut coeffs = {
        let mut __v = Vec::new();
        resize_numbers(&mut __v, max + 1, &Number::integer(Integer::zero()));
        __v
    };
    for term in poly.terms() {
        let d = term.exponents()[var] as usize;
        coeffs[d] = clone_number(term.coefficient());
    }
    Ok(trim_dense(&coeffs))
}

fn from_dense(coeffs: &[Number], var: usize, n: usize, ring: RingId, rings: &RingTable) -> Result<Polynomial> {
    let mut b = PolynomialBuilder::new(ring);
    for (d, c) in coeffs.iter().enumerate() {
        if c.is_zero() {
            continue;
        }
        let mut exp = vec![0u32; n];
        exp[var] = d as u32;
        b.push_term(clone_number(&c), exp)?;
    }
    b.build(rings)
}

fn trim_dense(v: &[Number]) -> Vec<Number> {
    let mut out = clone_numbers(v);
    while out.len() > 1 && out.last().is_some_and(|c| c.is_zero()) {
        out.pop();
    }
    if out.len() == 1 && out[0].is_zero() {
        return vec![Number::small_int(0)];
    }
    if out.is_empty() { vec![Number::small_int(0)] } else { out }
}

fn is_zero_dense(v: &[Number]) -> bool {
    v.is_empty() || v.iter().all(|c| c.is_zero())
}

fn degree(v: &[Number]) -> usize {
    if is_zero_dense(v) {
        return 0;
    }
    v.len() - 1
}

fn lc(v: &[Number]) -> Result<Number> {
    v.last()
        .map(clone_number)
        .ok_or_else(|| Diagnostic::new(DiagnosticCode::DomainError).detail("domain", "polynomial").detail("operation", "zero_polynomial"))
}

fn sub_scaled_monomial(a: &[Number], b: &[Number], scale: Number, shift: usize, coeff: &CoefficientRing<'_>) -> Result<Vec<Number>> {
    let mut out = clone_numbers(a);
    for (i, bc) in b.iter().enumerate() {
        let idx = i + shift;
        if idx >= out.len() {
            resize_numbers(&mut out, idx + 1, &Number::integer(Integer::zero()));
        }
        let sub = coeff.mul(clone_number(&scale), clone_number(&bc))?;
        out[idx] = coeff.sub(clone_number(&out[idx]), sub)?;
    }
    Ok(trim_dense(&out))
}

fn sub_scaled_monomial_rational(a: &[Number], b: &[Number], scale: Number, shift: usize) -> Result<Vec<Number>> {
    let mut out = clone_numbers(a);
    for (i, bc) in b.iter().enumerate() {
        let idx = i + shift;
        if idx >= out.len() {
            resize_numbers(&mut out, idx + 1, &Number::integer(Integer::zero()));
        }
        let sub = num_mul(clone_number(&scale), clone_number(&bc))?;
        out[idx] = num_add(clone_number(&out[idx]), num_neg(sub))?;
    }
    Ok(trim_dense(&out))
}

fn scale_dense(v: &[Number], scale: Number) -> Result<Vec<Number>> {
    v.iter().map(|c| num_mul(clone_number(&c), clone_number(&scale))).collect()
}

fn content_dense(v: &[Number]) -> Result<Integer> {
    let mut g = Integer::zero();
    for c in v {
        if c.is_zero() {
            continue;
        }
        let i = match c {
            Number::Integer(n) => clone_integer(n),
            Number::Rational(r) if r.is_integer() => clone_integer(&r.numerator()),
            _ => {
                return Err(Diagnostic::new(DiagnosticCode::NumericDomainMismatch)
                    .detail("domain", "polynomial")
                    .detail("operation", "integer_content"));
            }
        };
        g = if g.is_zero() { i.abs() } else { g.gcd(&i.abs()) };
    }
    Ok(if g.is_zero() { Integer::zero() } else { g })
}

fn primitive_part_dense(v: &[Number], content: &Integer) -> Result<Vec<Number>> {
    if content.is_zero() {
        return Ok(vec![Number::small_int(0)]);
    }
    v.iter()
        .map(|c| match c {
            Number::Integer(n) => Ok(Number::integer(n.div(content).expect("div"))),
            _ => {
                Err(Diagnostic::new(DiagnosticCode::NumericDomainMismatch).detail("domain", "polynomial").detail("operation", "primitive_part"))
            }
        })
        .collect()
}

fn monic_dense(v: &[Number], coeff: &CoefficientRing<'_>) -> Result<Vec<Number>> {
    if is_zero_dense(v) {
        return Ok(clone_numbers(v));
    }
    let lc_inv = coeff.inv(lc(v)?)?;
    v.iter().map(|c| coeff.mul(clone_number(&c), clone_number(&lc_inv))).collect()
}

fn monic_dense_rational(v: &[Number]) -> Result<Vec<Number>> {
    if is_zero_dense(v) {
        return Ok(clone_numbers(v));
    }
    let inv = num_div(Number::small_int(1), lc(v)?)?;
    scale_dense(v, inv)
}

fn ensure_integer_coeffs(v: &[Number]) -> Result<()> {
    for c in v {
        match c {
            Number::Integer(_) => {}
            Number::Rational(r) if r.is_integer() => {}
            _ => {
                return Err(Diagnostic::new(DiagnosticCode::PolynomialNonFieldDivision)
                    .detail("domain", "polynomial")
                    .detail("operation", "non_integer_coefficient"));
            }
        }
    }
    Ok(())
}

fn num_pow(base: Number, exp: usize) -> Result<Number> {
    let mut acc = Number::small_int(1);
    for _ in 0..exp {
        acc = num_mul(acc, clone_number(&base))?;
    }
    Ok(acc)
}

fn ensure_same_ring(lhs: &Polynomial, rhs: &Polynomial) -> Result<()> {
    if lhs.ring() != rhs.ring() {
        return Err(Diagnostic::new(DiagnosticCode::DomainMismatch).detail("domain", "polynomial").detail("operation", "ring_mismatch"));
    }
    Ok(())
}

fn division_by_zero() -> Diagnostic {
    Diagnostic::new(DiagnosticCode::PolynomialDivisionByZero).detail("domain", "polynomial")
}

fn exponent_mismatch() -> Diagnostic {
    Diagnostic::new(DiagnosticCode::PolynomialVariableMismatch).detail("domain", "polynomial").detail("operation", "exponent_length")
}

fn unsupported_domain() -> Diagnostic {
    Diagnostic::new(DiagnosticCode::UnsupportedOperation).detail("domain", "polynomial").detail("operation", "univariate_unsupported_domain")
}

fn ring_unknown(ring: RingId) -> Diagnostic {
    Diagnostic::new(DiagnosticCode::UnsupportedOperation)
        .detail("domain", "polynomial")
        .detail("operation", "unknown_ring")
        .detail("ring_id", ring.0.to_string())
}
