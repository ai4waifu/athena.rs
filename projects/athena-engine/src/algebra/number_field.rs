//! 数域 `$\mathbb{Q}(\alpha)$` 幂基与相对扩张算术。

use athena_numeric::{Integer, Rational};
use athena_types::{Diagnostic, DiagnosticCode, ExtensionId, FieldId, Result};

/// 数域幂基 / 相对塔规格。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NumberFieldSpec {
    /// 扩张 id。
    pub extension: ExtensionId,
    /// 相对基域 K。
    pub base: FieldId,
    /// 绝对基域（`$\mathbb{Q}$`）。
    pub absolute_base: FieldId,
    /// 相对次数 `[L:K]`。
    pub relative_degree: u32,
    /// 绝对次数 `[L:\mathbb{Q}]`。
    pub absolute_degree: u32,
    /// 相对定义多项式（系数取自 K 的绝对坐标，升幂首一）。
    pub relative_modulus: Vec<Vec<Rational>>,
    /// 绝对定义 / 极小多项式（升幂首一，系数 `$\in\mathbb{Q}$`）。
    pub absolute_modulus: Vec<Rational>,
}

/// 首一化有理多项式。
pub fn make_monic(mut coeffs: Vec<Rational>) -> Result<Vec<Rational>> {
    while coeffs.last().is_some_and(|c| c.is_zero()) {
        coeffs.pop();
    }
    if coeffs.len() < 2 {
        return Err(ext_err("modulus_degree_too_small"));
    }
    let lead = coeffs.last().cloned().ok_or_else(|| ext_err("empty_modulus"))?;
    if lead.is_zero() {
        return Err(ext_err("modulus_leading_zero"));
    }
    if lead != Rational::one() {
        for c in &mut coeffs {
            *c = c.try_div(&lead).map_err(|_| ext_err("modulus_make_monic"))?;
        }
    }
    Ok(coeffs)
}

/// 校验首一并返回次数。
pub fn validate_rational_modulus(coeffs: &[Rational]) -> Result<u32> {
    if coeffs.len() < 2 || coeffs.last() != Some(&Rational::one()) {
        return Err(ext_err("modulus_not_monic"));
    }
    u32::try_from(coeffs.len() - 1).map_err(|_| ext_err("modulus_degree_overflow"))
}

/// `$\mathbb{Q}[x]$` 不可约性（首一）。
pub fn is_irreducible_over_rationals(coeffs: &[Rational]) -> Result<bool> {
    let n = validate_rational_modulus(coeffs)? as usize;
    if n <= 1 {
        return Ok(n == 1);
    }
    let z = to_primitive_z(coeffs)?;
    if has_rational_root(&z) {
        return Ok(false);
    }
    if n == 2 || n == 3 {
        return Ok(true);
    }
    if is_eisenstein(&z) {
        return Ok(true);
    }
    for d in 1..=(n / 2) {
        if has_factor_degree(&z, d) {
            return Ok(false);
        }
    }
    Ok(true)
}

/// 规范绝对坐标长度。
pub fn canonical_nf_coords(mut coords: Vec<Rational>, degree: u32) -> Result<Vec<Rational>> {
    let n = degree as usize;
    if coords.len() > n {
        return Err(elem_err("nf_coord_length"));
    }
    coords.resize(n, Rational::zero());
    Ok(coords)
}

/// 坐标加法。
pub fn add_nf_coords(a: &[Rational], b: &[Rational]) -> Vec<Rational> {
    a.iter().zip(b.iter()).map(|(x, y)| x.add(y)).collect()
}

/// 绝对坐标乘法。
pub fn mul_nf_coords(a: &[Rational], b: &[Rational], modulus: &[Rational]) -> Vec<Rational> {
    poly_mod_q(&poly_mul_q(a, b), modulus)
}

/// 绝对坐标逆元。
pub fn inv_nf_coords(a: &[Rational], modulus: &[Rational]) -> Result<Vec<Rational>> {
    if a.iter().all(|c| c.is_zero()) {
        return Err(Diagnostic::new(DiagnosticCode::DivideByZero).detail("domain", "field"));
    }
    let (_, s, _) = poly_egcd_q(a, modulus)?;
    Ok(poly_mod_q(&s, modulus))
}

/// 基域坐标嵌入扩张（常数项块）。
pub fn embed_base_coords(base_coords: &[Rational], absolute_degree: u32, base_degree: u32) -> Result<Vec<Rational>> {
    if base_coords.len() != base_degree as usize {
        return Err(elem_err("embed_base_length"));
    }
    let mut out = vec![Rational::zero(); absolute_degree as usize];
    out[..base_coords.len()].clone_from_slice(base_coords);
    Ok(out)
}

/// 相对模（有理系数）写成基域坐标块。
pub fn relative_modulus_from_rational(coeffs: &[Rational], base_degree: u32) -> Result<Vec<Vec<Rational>>> {
    let monic = make_monic(coeffs.to_vec())?;
    validate_rational_modulus(&monic)?;
    let bd = base_degree as usize;
    Ok(monic
        .into_iter()
        .map(|c| {
            let mut block = vec![Rational::zero(); bd];
            block[0] = c;
            block
        })
        .collect())
}

/// 绝对次数乘积。
pub fn absolute_degree_product(base_degree: u32, relative_degree: u32) -> Result<u32> {
    base_degree.checked_mul(relative_degree).ok_or_else(|| ext_err("absolute_degree_overflow"))
}

/// 由幂次求最短首一关系。
pub fn minimal_polynomial_from_powers(powers: &[Vec<Rational>]) -> Result<Vec<Rational>> {
    let n = powers.len().saturating_sub(1);
    for deg in 1..=n {
        if let Some(rel) = shortest_relation(&powers[..=deg])? {
            return make_monic(rel);
        }
    }
    Err(elem_err("minpoly_not_found"))
}

/// 绝对情形极小多项式。
pub fn minimal_polynomial_over_q(coords: &[Rational], modulus: &[Rational]) -> Result<Vec<Rational>> {
    let n = modulus.len().saturating_sub(1);
    if coords.len() != n {
        return Err(elem_err("minpoly_coord_length"));
    }
    if coords.iter().all(|c| c.is_zero()) {
        return Ok(vec![Rational::zero(), Rational::one()]);
    }
    let mut powers = Vec::with_capacity(n + 1);
    let mut cur = {
        let mut one = vec![Rational::zero(); n];
        one[0] = Rational::one();
        one
    };
    for _ in 0..=n {
        powers.push(cur.clone());
        cur = mul_nf_coords(&cur, coords, modulus);
    }
    minimal_polynomial_from_powers(&powers)
}

/// 相对乘法（展平坐标）。
pub fn mul_relative_nf_coords(
    a: &[Rational],
    b: &[Rational],
    base_modulus: &[Rational],
    relative_modulus: &[Vec<Rational>],
    base_degree: u32,
    relative_degree: u32,
) -> Result<Vec<Rational>> {
    let bd = base_degree as usize;
    let rd = relative_degree as usize;
    let abs = bd * rd;
    if a.len() != abs || b.len() != abs {
        return Err(elem_err("relative_mul_length"));
    }
    let a_poly = split_blocks(a, bd, rd);
    let b_poly = split_blocks(b, bd, rd);
    let mut prod = vec![vec![Rational::zero(); bd]; 2 * rd];
    for i in 0..rd {
        for j in 0..rd {
            let p = mul_nf_coords(&a_poly[i], &b_poly[j], base_modulus);
            prod[i + j] = add_nf_coords(&prod[i + j], &p);
        }
    }
    let g = relative_modulus;
    let dg = g.len() - 1;
    while prod.len() > dg {
        let lead = prod.last().cloned().unwrap_or_else(|| vec![Rational::zero(); bd]);
        if lead.iter().all(|c| c.is_zero()) {
            prod.pop();
            continue;
        }
        let k = prod.len() - g.len();
        for (i, coeff_block) in g.iter().enumerate() {
            let scaled = mul_nf_coords(&lead, coeff_block, base_modulus);
            prod[k + i] = add_nf_coords(&prod[k + i], &scaled.iter().map(|c| c.neg()).collect::<Vec<_>>());
        }
        while prod.last().is_some_and(|block| block.iter().all(|c| c.is_zero())) {
            prod.pop();
        }
    }
    prod.resize(rd, vec![Rational::zero(); bd]);
    Ok(flatten_blocks(&prod))
}

/// 相对逆元（线性求解乘法映射）。
pub fn inv_relative_nf_coords(
    a: &[Rational],
    base_modulus: &[Rational],
    relative_modulus: &[Vec<Rational>],
    base_degree: u32,
    relative_degree: u32,
) -> Result<Vec<Rational>> {
    let abs = (base_degree * relative_degree) as usize;
    if a.iter().all(|c| c.is_zero()) {
        return Err(Diagnostic::new(DiagnosticCode::DivideByZero).detail("domain", "field"));
    }
    let mut one = vec![Rational::zero(); abs];
    one[0] = Rational::one();
    let mut cols = Vec::with_capacity(abs);
    let mut e = vec![Rational::zero(); abs];
    for i in 0..abs {
        e[i] = Rational::one();
        if i > 0 {
            e[i - 1] = Rational::zero();
        }
        cols.push(mul_relative_nf_coords(&e, a, base_modulus, relative_modulus, base_degree, relative_degree)?);
        e[i] = Rational::zero();
    }
    solve_linear_q(&cols, &one)
}

fn ext_err(op: &'static str) -> Diagnostic {
    Diagnostic::new(DiagnosticCode::FieldExtensionInvalid).detail("domain", "field").detail("operation", op)
}

fn elem_err(op: &'static str) -> Diagnostic {
    Diagnostic::new(DiagnosticCode::FieldElementInvalid).detail("domain", "field").detail("operation", op)
}

fn lcm_int(a: &Integer, b: &Integer) -> Integer {
    if a.is_zero() || b.is_zero() {
        return Integer::zero();
    }
    a.mul(b).div(&a.gcd(b)).abs()
}

fn to_primitive_z(coeffs: &[Rational]) -> Result<Vec<Integer>> {
    let mut dens = Integer::one();
    for c in coeffs {
        dens = lcm_int(&dens, &c.denominator());
    }
    let mut ints: Vec<Integer> = coeffs.iter().map(|c| c.numerator().mul(&dens.div(&c.denominator()))).collect();
    let mut g = Integer::zero();
    for c in &ints {
        g = if g.is_zero() { c.abs() } else { g.gcd(&c.abs()) };
    }
    if !g.is_zero() && !g.is_one() {
        for c in &mut ints {
            *c = c.div(&g);
        }
    }
    Ok(ints)
}

fn has_rational_root(z: &[Integer]) -> bool {
    let n = z.len() - 1;
    let constant = z[0].clone();
    let lead = z[n].clone();
    if constant.is_zero() {
        return true;
    }
    for p in small_divisors(&constant) {
        for q in small_divisors(&lead) {
            for sign in [1i32, -1] {
                let numer = if sign > 0 { p.clone() } else { p.neg() };
                if eval_z(z, &numer, &q).is_zero() {
                    return true;
                }
            }
        }
    }
    false
}

fn eval_z(z: &[Integer], numer: &Integer, denom: &Integer) -> Integer {
    let deg = z.len() - 1;
    let mut acc = Integer::zero();
    for (i, c) in z.iter().enumerate() {
        let pn = numer.pow_u32(i as u32).unwrap_or_else(|_| Integer::zero());
        let pd = denom.pow_u32((deg - i) as u32).unwrap_or_else(|_| Integer::zero());
        acc = acc.add(&c.mul(&pn).mul(&pd));
    }
    acc
}

fn small_divisors(n: &Integer) -> Vec<Integer> {
    let abs = n.abs();
    let mut out = Vec::new();
    let mut d = Integer::one();
    while d.cmp(&abs) != std::cmp::Ordering::Greater {
        if abs.rem(&d).is_zero() {
            out.push(d.clone());
        }
        d = d.add(&Integer::one());
        if out.len() > 64 || d.bits() > 16 {
            break;
        }
    }
    if out.is_empty() {
        out.push(Integer::one());
    }
    out
}

fn is_eisenstein(z: &[Integer]) -> bool {
    let n = z.len() - 1;
    if n < 2 || !z[n].is_one() {
        return false;
    }
    let c0 = z[0].abs();
    if c0.is_zero() {
        return false;
    }
    for p_cand in [2i64, 3, 5, 7, 11, 13, 17, 19, 23] {
        let p = Integer::from_i64(p_cand);
        if !c0.rem(&p).is_zero() || z[n].rem(&p).is_zero() {
            continue;
        }
        if z[..n].iter().any(|c| !c.rem(&p).is_zero()) {
            continue;
        }
        if c0.rem(&p.mul(&p)).is_zero() {
            continue;
        }
        return true;
    }
    false
}

fn has_factor_degree(z: &[Integer], degree: usize) -> bool {
    if degree == 0 || degree >= z.len() {
        return false;
    }
    let bound = 3i64;
    let mut coeffs = vec![Integer::zero(); degree + 1];
    coeffs[degree] = Integer::one();
    loop {
        if divides_z(z, &coeffs) {
            return true;
        }
        if !inc_bound(&mut coeffs[..degree], bound) {
            break;
        }
    }
    false
}

fn divides_z(f: &[Integer], g: &[Integer]) -> bool {
    let (_, rem) = div_rem_z(f, g);
    rem.iter().all(|c| c.is_zero())
}

fn div_rem_z(a: &[Integer], b: &[Integer]) -> (Vec<Integer>, Vec<Integer>) {
    let mut rem: Vec<Integer> = a.to_vec();
    while rem.last().is_some_and(|c| c.is_zero()) {
        rem.pop();
    }
    let mut bb = b.to_vec();
    while bb.last().is_some_and(|c| c.is_zero()) {
        bb.pop();
    }
    if bb.is_empty() || rem.len() < bb.len() {
        return (Vec::new(), rem);
    }
    let db = bb.len() - 1;
    let lb = bb[db].clone();
    let mut quot = vec![Integer::zero(); rem.len() - bb.len() + 1];
    while rem.len() >= bb.len() {
        let dr = rem.len() - 1;
        let lr = rem[dr].clone();
        if !lr.rem(&lb).is_zero() {
            return (Vec::new(), a.to_vec());
        }
        let q = lr.div(&lb);
        let pos = dr - db;
        quot[pos] = q.clone();
        for i in 0..=db {
            rem[pos + i] = rem[pos + i].sub(&q.mul(&bb[i]));
        }
        while rem.last().is_some_and(|c| c.is_zero()) {
            rem.pop();
        }
        if rem.is_empty() {
            break;
        }
    }
    (quot, rem)
}

fn inc_bound(coeffs: &mut [Integer], bound: i64) -> bool {
    for c in coeffs.iter_mut() {
        let v = c.to_i64().unwrap_or(bound);
        if v < bound {
            *c = Integer::from_i64(v + 1);
            return true;
        }
        *c = Integer::from_i64(-bound);
    }
    false
}

fn poly_mul_q(a: &[Rational], b: &[Rational]) -> Vec<Rational> {
    if a.is_empty() || b.is_empty() {
        return Vec::new();
    }
    let mut out = vec![Rational::zero(); a.len() + b.len() - 1];
    for (i, x) in a.iter().enumerate() {
        if x.is_zero() {
            continue;
        }
        for (j, y) in b.iter().enumerate() {
            if y.is_zero() {
                continue;
            }
            out[i + j] = out[i + j].add(&x.mul(y));
        }
    }
    out
}

fn poly_mod_q(a: &[Rational], m: &[Rational]) -> Vec<Rational> {
    let mut rem = a.to_vec();
    let dm = m.len() - 1;
    while rem.len() > dm {
        let lr = rem.last().cloned().unwrap_or_else(Rational::zero);
        if lr.is_zero() {
            rem.pop();
            continue;
        }
        let k = rem.len() - m.len();
        for i in 0..m.len() {
            rem[k + i] = rem[k + i].sub(&lr.mul(&m[i]));
        }
        while rem.last().is_some_and(|c| c.is_zero()) {
            rem.pop();
        }
    }
    rem.resize(dm, Rational::zero());
    rem
}

fn poly_egcd_q(a: &[Rational], b: &[Rational]) -> Result<(Vec<Rational>, Vec<Rational>, Vec<Rational>)> {
    let mut r0 = trim_q(a.to_vec());
    let mut r1 = trim_q(b.to_vec());
    let mut s0 = vec![Rational::one()];
    let mut s1 = vec![Rational::zero()];
    let mut t0 = vec![Rational::zero()];
    let mut t1 = vec![Rational::one()];
    while !r1.iter().all(|c| c.is_zero()) {
        let (q, r) = poly_div_rem_q(&r0, &r1)?;
        let ns = poly_sub_q(&s0, &poly_mul_q(&q, &s1));
        let nt = poly_sub_q(&t0, &poly_mul_q(&q, &t1));
        r0 = r1;
        r1 = r;
        s0 = s1;
        s1 = ns;
        t0 = t1;
        t1 = nt;
    }
    let lead = r0.last().cloned().unwrap_or_else(Rational::one);
    if lead.is_zero() {
        return Err(elem_err("nf_gcd_zero"));
    }
    if lead != Rational::one() {
        r0 = r0.into_iter().map(|c| c.try_div(&lead).unwrap_or_else(|_| Rational::zero())).collect();
        s0 = s0.into_iter().map(|c| c.try_div(&lead).unwrap_or_else(|_| Rational::zero())).collect();
        t0 = t0.into_iter().map(|c| c.try_div(&lead).unwrap_or_else(|_| Rational::zero())).collect();
    }
    if r0 != [Rational::one()] {
        return Err(elem_err("nf_not_invertible"));
    }
    Ok((r0, s0, t0))
}

fn poly_div_rem_q(a: &[Rational], b: &[Rational]) -> Result<(Vec<Rational>, Vec<Rational>)> {
    let mut rem = trim_q(a.to_vec());
    let b = trim_q(b.to_vec());
    if b.is_empty() {
        return Err(elem_err("poly_div_by_zero"));
    }
    let db = b.len() - 1;
    let lb = b[db].clone();
    if rem.len() < b.len() {
        return Ok((Vec::new(), rem));
    }
    let mut quot = vec![Rational::zero(); rem.len() - b.len() + 1];
    while rem.len() >= b.len() {
        let dr = rem.len() - 1;
        let lr = rem[dr].clone();
        if lr.is_zero() {
            rem.pop();
            continue;
        }
        let q = lr.try_div(&lb).map_err(|_| elem_err("poly_div"))?;
        let pos = dr - db;
        quot[pos] = q.clone();
        for i in 0..=db {
            rem[pos + i] = rem[pos + i].sub(&q.mul(&b[i]));
        }
        while rem.last().is_some_and(|c| c.is_zero()) {
            rem.pop();
        }
        if rem.is_empty() {
            break;
        }
    }
    Ok((quot, rem))
}

fn poly_sub_q(a: &[Rational], b: &[Rational]) -> Vec<Rational> {
    let n = a.len().max(b.len());
    let mut out = vec![Rational::zero(); n];
    for i in 0..n {
        let x = a.get(i).cloned().unwrap_or_else(Rational::zero);
        let y = b.get(i).cloned().unwrap_or_else(Rational::zero);
        out[i] = x.sub(&y);
    }
    trim_q(out)
}

fn trim_q(mut v: Vec<Rational>) -> Vec<Rational> {
    while v.last().is_some_and(|c| c.is_zero()) {
        v.pop();
    }
    v
}

fn shortest_relation(powers: &[Vec<Rational>]) -> Result<Option<Vec<Rational>>> {
    let cols = powers.len();
    let rows = powers[0].len();
    let mut mat = vec![vec![Rational::zero(); cols]; rows];
    for j in 0..cols {
        for i in 0..rows {
            mat[i][j] = powers[j][i].clone();
        }
    }
    let mut col = 0usize;
    for row in 0..rows {
        while col < cols {
            let mut pivot = None;
            for r in row..rows {
                if !mat[r][col].is_zero() {
                    pivot = Some(r);
                    break;
                }
            }
            let Some(pr) = pivot else {
                col += 1;
                continue;
            };
            if pr != row {
                mat.swap(pr, row);
            }
            let pv = mat[row][col].clone();
            for c in col..cols {
                mat[row][c] = mat[row][c].try_div(&pv).map_err(|_| elem_err("rref_div"))?;
            }
            for r in 0..rows {
                if r == row {
                    continue;
                }
                let f = mat[r][col].clone();
                if f.is_zero() {
                    continue;
                }
                for c in col..cols {
                    mat[r][c] = mat[r][c].sub(&f.mul(&mat[row][c]));
                }
            }
            col += 1;
            break;
        }
    }
    let mut used = vec![false; cols];
    for row in 0..rows {
        if let Some(p) = (0..cols).find(|&c| !mat[row][c].is_zero()) {
            used[p] = true;
        }
    }
    let free: Vec<usize> = (0..cols).filter(|&j| !used[j]).collect();
    if free.is_empty() {
        return Ok(None);
    }
    let f = *free.last().unwrap();
    let mut rel = vec![Rational::zero(); cols];
    rel[f] = Rational::one();
    for row in 0..rows {
        let piv = (0..cols).find(|&c| !mat[row][c].is_zero());
        let Some(p) = piv else {
            continue;
        };
        let mut val = Rational::zero();
        for j in (p + 1)..cols {
            val = val.add(&mat[row][j].mul(&rel[j]));
        }
        rel[p] = val.neg();
    }
    while rel.last().is_some_and(|c| c.is_zero()) {
        rel.pop();
    }
    if rel.len() < 2 {
        return Ok(None);
    }
    Ok(Some(rel))
}

fn split_blocks(v: &[Rational], bd: usize, rd: usize) -> Vec<Vec<Rational>> {
    (0..rd).map(|i| v[i * bd..(i + 1) * bd].to_vec()).collect()
}

fn flatten_blocks(blocks: &[Vec<Rational>]) -> Vec<Rational> {
    blocks.iter().flatten().cloned().collect()
}

fn solve_linear_q(columns: &[Vec<Rational>], target: &[Rational]) -> Result<Vec<Rational>> {
    let n = columns.len();
    let mut mat = vec![vec![Rational::zero(); n + 1]; n];
    for j in 0..n {
        for i in 0..n {
            mat[i][j] = columns[j][i].clone();
        }
    }
    for i in 0..n {
        mat[i][n] = target[i].clone();
    }
    for col in 0..n {
        let mut pivot = None;
        for r in col..n {
            if !mat[r][col].is_zero() {
                pivot = Some(r);
                break;
            }
        }
        let Some(pr) = pivot else {
            return Err(elem_err("nf_singular"));
        };
        if pr != col {
            mat.swap(pr, col);
        }
        let pv = mat[col][col].clone();
        for c in col..=n {
            mat[col][c] = mat[col][c].try_div(&pv).map_err(|_| elem_err("nf_solve_div"))?;
        }
        for r in 0..n {
            if r == col {
                continue;
            }
            let f = mat[r][col].clone();
            if f.is_zero() {
                continue;
            }
            for c in col..=n {
                mat[r][c] = mat[r][c].sub(&f.mul(&mat[col][c]));
            }
        }
    }
    Ok((0..n).map(|i| mat[i][n].clone()).collect())
}
