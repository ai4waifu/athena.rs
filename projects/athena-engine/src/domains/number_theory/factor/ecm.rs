//! ECM stage 1（Montgomery 曲线，纯 Rust 引导实现）。

use crate::runtime::values::numeric_clone::clone_integer;
use athena_numeric::Integer;

/// Stage 1：在若干条 Montgomery 曲线上计算 `[k]P`，用 `gcd(Z, n)` 探测因子。
pub fn ecm_stage_one(n: &Integer, seed: u64, b1: u32, max_curves: u32) -> Option<Integer> {
    if n.is_one() || !n.is_odd() {
        return None;
    }
    let k = smooth_exponent(b1);
    for i in 0..max_curves {
        let sigma = Integer::from_u64(6 + seed.wrapping_add(u64::from(i)).wrapping_mul(0x9E37_79B9) % 50_000);
        if let Some(d) = try_curve(n, &sigma, &k) {
            return Some(d);
        }
    }
    None
}

fn try_curve(n: &Integer, sigma: &Integer, k: &Integer) -> Option<Integer> {
    // Suyama：u = σ²−5, v = 4σ；a24 = (A+2)/4 = (v−u)³(3u+v)/(16u³v)
    let five = Integer::from_i64(5);
    let four = Integer::from_i64(4);
    let three = Integer::from_i64(3);
    let sixteen = Integer::from_i64(16);

    let u = sigma.mul(sigma).sub(&five).rem(n).expect("rem");
    let v = four.mul(sigma).rem(n).expect("rem");
    if u.is_zero() || v.is_zero() {
        return None;
    }

    let u3 = u.mul(&u).mul(&u).rem(n).expect("rem");
    let v_minus_u = v.sub(&u).rem(n).expect("rem");
    let three_u_plus_v = three.mul(&u).add(&v).rem(n).expect("rem");
    let num = v_minus_u.mul(&v_minus_u).mul(&v_minus_u).mul(&three_u_plus_v).rem(n).expect("rem");
    let den = sixteen.mul(&u3).mul(&v).rem(n).expect("rem");
    if let Some(d) = nontrivial_gcd(&den, n) {
        return Some(d);
    }
    let inv_den = mod_inv(&den, n)?;
    let a24 = num.mul(&inv_den).rem(n).expect("rem");

    let v3 = v.mul(&v).mul(&v).rem(n).expect("rem");
    if let Some(d) = nontrivial_gcd(&v3, n) {
        return Some(d);
    }
    let inv_v3 = mod_inv(&v3, n)?;
    let x0 = u3.mul(&inv_v3).rem(n).expect("rem");
    let z0 = Integer::one();

    let (_xk, zk) = scalar_mul_montgomery(n, &a24, &x0, &z0, k);
    nontrivial_gcd(&zk, n)
}

fn nontrivial_gcd(a: &Integer, n: &Integer) -> Option<Integer> {
    let g = a.gcd(n);
    if !g.is_one() && g != *n { Some(g) } else { None }
}

fn scalar_mul_montgomery(n: &Integer, a24: &Integer, x: &Integer, z: &Integer, k: &Integer) -> (Integer, Integer) {
    let mut r0x = Integer::one();
    let mut r0z = Integer::zero();
    let mut r1x = clone_integer(&x);
    let mut r1z = clone_integer(&z);

    let bits = k.bits();
    if bits == 0 {
        return (r0x, r0z);
    }
    for i in (0..bits).rev() {
        if bit_at(k, i) {
            let (sx, sz) = mont_add(n, &r0x, &r0z, &r1x, &r1z, x, z);
            let (dx, dz) = mont_dbl(n, a24, &r1x, &r1z);
            r0x = sx;
            r0z = sz;
            r1x = dx;
            r1z = dz;
        }
        else {
            let (sx, sz) = mont_add(n, &r0x, &r0z, &r1x, &r1z, x, z);
            let (dx, dz) = mont_dbl(n, a24, &r0x, &r0z);
            r1x = sx;
            r1z = sz;
            r0x = dx;
            r0z = dz;
        }
    }
    (r0x, r0z)
}

fn bit_at(k: &Integer, i: u64) -> bool {
    let two = Integer::from_i64(2);
    let mut t = clone_integer(&k);
    for _ in 0..i {
        t = t.div(&two).expect("div");
    }
    !t.rem(&two).expect("rem").is_zero()
}

fn mont_dbl(n: &Integer, a24: &Integer, x: &Integer, z: &Integer) -> (Integer, Integer) {
    let xpz = x.add(z).rem(n).expect("rem");
    let xmz = x.sub(z).rem(n).expect("rem");
    let xpz2 = xpz.mul(&xpz).rem(n).expect("rem");
    let xmz2 = xmz.mul(&xmz).rem(n).expect("rem");
    let t = xpz2.sub(&xmz2).rem(n).expect("rem");
    let x2 = xpz2.mul(&xmz2).rem(n).expect("rem");
    let z2 = t.mul(&xmz2.add(&a24.mul(&t).rem(n).expect("rem")).rem(n).expect("rem")).rem(n).expect("rem");
    (x2, z2)
}

fn mont_add(
    n: &Integer,
    x2: &Integer,
    z2: &Integer,
    x3: &Integer,
    z3: &Integer,
    x1: &Integer,
    z1: &Integer,
) -> (Integer, Integer) {
    let a = x2.sub(z2).rem(n).expect("rem");
    let b = x2.add(z2).rem(n).expect("rem");
    let c = x3.sub(z3).rem(n).expect("rem");
    let d = x3.add(z3).rem(n).expect("rem");
    let da = d.mul(&a).rem(n).expect("rem");
    let cb = c.mul(&b).rem(n).expect("rem");
    let sum = da.add(&cb).rem(n).expect("rem");
    let diff = da.sub(&cb).rem(n).expect("rem");
    let x5 = z1.mul(&sum.mul(&sum).rem(n).expect("rem")).rem(n).expect("rem");
    let z5 = x1.mul(&diff.mul(&diff).rem(n).expect("rem")).rem(n).expect("rem");
    (x5, z5)
}

fn mod_inv(a: &Integer, n: &Integer) -> Option<Integer> {
    if !a.gcd(n).is_one() {
        return None;
    }
    let mut t = Integer::zero();
    let mut newt = Integer::one();
    let mut r = clone_integer(&n);
    let mut newr = a.rem(n).expect("rem");
    while !newr.is_zero() {
        let q = r.div(&newr).expect("div");
        let tmp_t = t.sub(&q.mul(&newt));
        t = newt;
        newt = tmp_t;
        let tmp_r = r.sub(&q.mul(&newr));
        r = newr;
        newr = tmp_r;
    }
    if t.is_negative() {
        t = t.add(n);
    }
    Some(t.rem(n).expect("rem"))
}

fn smooth_exponent(b1: u32) -> Integer {
    let mut k = Integer::one();
    for p in primes_up_to(b1) {
        let mut pp = p;
        while pp <= b1 {
            k = k.mul(&Integer::from_u64(u64::from(p)));
            let next = pp.saturating_mul(p);
            if next <= pp {
                break;
            }
            pp = next;
        }
    }
    k
}

fn primes_up_to(b1: u32) -> Vec<u32> {
    if b1 < 2 {
        return Vec::new();
    }
    let mut sieve = vec![true; (b1 as usize) + 1];
    sieve[0] = false;
    sieve[1] = false;
    let limit = b1 as usize;
    for p in 2..=limit {
        if sieve[p] {
            let mut m = p.saturating_mul(p);
            while m <= limit {
                sieve[m] = false;
                m += p;
            }
        }
    }
    (2..=b1).filter(|&p| sieve[p as usize]).collect()
}
