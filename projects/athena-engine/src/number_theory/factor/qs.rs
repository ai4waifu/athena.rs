//! QS 阶段：Fermat 近距分裂 + Dixon random squares。

use athena_numeric::Integer;

use super::super::arithmetic::{isqrt, isqrt_if_exact, jacobi_symbol};

/// Fermat 方法：因子接近 `√n` 时有效。
pub fn fermat_split(n: &Integer, max_steps: u64) -> Option<Integer> {
    if n.is_one() {
        return None;
    }
    if n.rem(&Integer::from_i64(2)).is_zero() {
        return if *n > Integer::from_i64(2) {
            Some(Integer::from_i64(2))
        } else {
            None
        };
    }
    let mut a = isqrt(n);
    if a.mul(&a).cmp(n) == std::cmp::Ordering::Less {
        a = a.add(&Integer::one());
    }
    for _ in 0..max_steps {
        let b2 = a.mul(&a).sub(n);
        if let Some(b) = isqrt_if_exact(&b2) {
            let p = a.sub(&b).gcd(n);
            if !p.is_one() && p != *n {
                return Some(p);
            }
        }
        a = a.add(&Integer::one());
    }
    None
}

/// Dixon random squares：搜集 `x² ≡ Q (mod n)` 的因子基光滑关系，解 `GF(2)` 依赖得因子。
pub fn dixon_split(n: &Integer, seed: u64, max_steps: u64) -> Option<Integer> {
    if n.is_one() {
        return None;
    }
    if n.rem(&Integer::from_i64(2)).is_zero() {
        return if *n > Integer::from_i64(2) {
            Some(Integer::from_i64(2))
        } else {
            None
        };
    }
    if max_steps == 0 {
        return None;
    }

    let bits = n.bits().max(10);
    let fb_bound = ((bits as u32).saturating_mul(6)).max(40).min(400);
    let factor_base = build_factor_base(n, fb_bound);
    if factor_base.len() == 1 {
        let p = Integer::from_u64(u64::from(factor_base[0]));
        if n.rem(&p).is_zero() && p != *n {
            return Some(p);
        }
    }
    if factor_base.is_empty() {
        return None;
    }
    let ncols = factor_base.len();

    let mut relations: Vec<Relation> = Vec::new();
    let mut pivots: Vec<Option<usize>> = vec![None; ncols];
    let mut reduced: Vec<(Vec<u8>, Vec<usize>)> = Vec::new();

    let mut rng = seed | 1;
    let mut steps = 0u64;
    let target = ncols + 8;

    while steps < max_steps {
        steps += 1;
        rng = rng
            .wrapping_mul(0x9E37_79B9_7F4A_7C15)
            .wrapping_add(0x517C_C1B7);
        let x = next_candidate(n, rng, steps);
        if x.is_zero() || x.is_one() {
            continue;
        }
        let g0 = x.gcd(n);
        if g0 > Integer::one() && g0 != *n {
            return Some(g0);
        }

        let q = x.mul(&x).rem(n);
        if q.is_zero() {
            continue;
        }
        let Some(exps) = factor_smooth(q, &factor_base) else {
            continue;
        };
        let parity: Vec<u8> = exps.iter().map(|e| (e & 1) as u8).collect();
        let rel_idx = relations.len();
        relations.push(Relation { x, exps });

        if let Some(comb) = insert_relation(&mut reduced, &mut pivots, parity, rel_idx) {
            if let Some(d) = dependency_factor(n, &factor_base, &relations, &comb) {
                return Some(d);
            }
        }

        if relations.len() >= target.saturating_mul(4) {
            // 关系过多仍无因子：扩大搜索无益，退出
            break;
        }
    }
    None
}

/// QS 组合入口：先短预算 Fermat，再 Dixon。
pub fn qs_split(n: &Integer, seed: u64, max_steps: u64) -> Option<Integer> {
    if max_steps == 0 {
        return None;
    }
    let fermat_budget = {
        let soft = (max_steps / 8).max(64);
        soft.min(max_steps.min(50_000))
    };
    if let Some(d) = fermat_split(n, fermat_budget) {
        return Some(d);
    }
    dixon_split(n, seed, max_steps.saturating_sub(fermat_budget))
}

struct Relation {
    x: Integer,
    exps: Vec<u32>,
}

fn build_factor_base(n: &Integer, bound: u32) -> Vec<u32> {
    let mut fb = Vec::new();
    if bound >= 2 {
        fb.push(2);
    }
    let mut p = 3u32;
    while p <= bound {
        if is_prime_u32(p) {
            match jacobi_symbol(n, &Integer::from_u64(u64::from(p))) {
                Some(0) => return vec![p],
                Some(1) => fb.push(p),
                Some(-1) if fb.len() < 12 => fb.push(p),
                _ => {}
            }
        }
        p = p.saturating_add(2);
        if p < 3 {
            break;
        }
    }
    fb
}

fn is_prime_u32(n: u32) -> bool {
    if n < 2 {
        return false;
    }
    if n % 2 == 0 {
        return n == 2;
    }
    let mut d = 3u32;
    while d.saturating_mul(d) <= n {
        if n % d == 0 {
            return false;
        }
        d += 2;
    }
    true
}

fn next_candidate(n: &Integer, rng: u64, step: u64) -> Integer {
    if step % 2 == 0 {
        isqrt(n).add(&Integer::one()).add(&Integer::from_u64(step / 2))
    } else if let Some(nv) = n.to_u64() {
        let m = nv.saturating_sub(2).max(2);
        Integer::from_u64(2 + (rng % m))
    } else {
        isqrt(n)
            .add(&Integer::one())
            .add(&Integer::from_u64((rng % 10_000).wrapping_add(step)))
    }
}

fn factor_smooth(mut q: Integer, fb: &[u32]) -> Option<Vec<u32>> {
    let mut exps = vec![0u32; fb.len()];
    for (i, &p) in fb.iter().enumerate() {
        let pb = Integer::from_u64(u64::from(p));
        while q.rem(&pb).is_zero() {
            q = q.div(&pb);
            exps[i] += 1;
            if q.is_one() {
                return Some(exps);
            }
        }
    }
    if q.is_one() {
        Some(exps)
    } else {
        None
    }
}

fn insert_relation(
    reduced: &mut Vec<(Vec<u8>, Vec<usize>)>,
    pivots: &mut [Option<usize>],
    mut parity: Vec<u8>,
    rel_idx: usize,
) -> Option<Vec<usize>> {
    let ncols = pivots.len();
    parity.resize(ncols, 0);
    let mut comb = vec![rel_idx];

    for c in 0..ncols {
        if parity[c] == 0 {
            continue;
        }
        if let Some(row_i) = pivots[c] {
            let (ref prow, ref pcomb) = reduced[row_i];
            for (a, b) in parity.iter_mut().zip(prow.iter()) {
                *a ^= *b;
            }
            comb = sym_diff(comb, pcomb);
        } else {
            pivots[c] = Some(reduced.len());
            reduced.push((parity, comb));
            return None;
        }
    }

    if comb.is_empty() {
        None
    } else {
        Some(comb)
    }
}

fn sym_diff(mut a: Vec<usize>, b: &[usize]) -> Vec<usize> {
    for &x in b {
        if let Some(pos) = a.iter().position(|&y| y == x) {
            a.swap_remove(pos);
        } else {
            a.push(x);
        }
    }
    a
}

fn dependency_factor(n: &Integer, factor_base: &[u32], relations: &[Relation], comb: &[usize]) -> Option<Integer> {
    if comb.is_empty() {
        return None;
    }
    let mut total = vec![0u32; factor_base.len()];
    let mut x_prod = Integer::one();
    for &i in comb {
        let rel = &relations[i];
        x_prod = x_prod.mul(&rel.x).rem(n);
        for (t, e) in total.iter_mut().zip(rel.exps.iter()) {
            *t = t.saturating_add(*e);
        }
    }
    if total.iter().any(|t| t % 2 != 0) {
        return None;
    }

    let mut y = Integer::one();
    for (j, &t) in total.iter().enumerate() {
        let half = t / 2;
        if half == 0 {
            continue;
        }
        let p = Integer::from_u64(u64::from(factor_base[j]));
        for _ in 0..half {
            y = y.mul(&p).rem(n);
        }
    }

    let diff = if x_prod >= y {
        x_prod.sub(&y)
    } else {
        y.sub(&x_prod)
    };
    let g = diff.gcd(n);
    if g > Integer::one() && g != *n {
        return Some(g);
    }
    let sum = x_prod.add(&y).rem(n);
    let g2 = sum.gcd(n);
    if g2 > Integer::one() && g2 != *n {
        return Some(g2);
    }
    None
}
