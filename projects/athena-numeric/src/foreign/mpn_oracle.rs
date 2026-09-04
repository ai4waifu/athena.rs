//! 独立 limb 级 mpn 参考实现（Living `13` foreign oracle）。
//!
//! **不进**默认 [`crate::kernel::KernelTable`]。仅用于差分 / fuzz：
//! Athena limbs 拷入本模块临时缓冲 → schoolbook 运算 → 拷回 `Vec<u64>`。
//!
//! 实现刻意不调用 `kernel::portable`，避免与生产路径同构冒充 oracle。

/// 去掉尾部零；全零返回空切片语义下的 `[0]` 规范化为单零 limb。
pub fn normalize(limbs: &[u64]) -> Vec<u64> {
    let mut n = limbs.len();
    while n > 0 && limbs[n - 1] == 0 {
        n -= 1;
    }
    if n == 0 {
        return vec![0];
    }
    limbs[..n].to_vec()
}

fn effective_len(limbs: &[u64]) -> usize {
    let mut n = limbs.len();
    while n > 0 && limbs[n - 1] == 0 {
        n -= 1;
    }
    n
}

fn is_zero(limbs: &[u64]) -> bool {
    effective_len(limbs) == 0
}

/// `out = a + b`（无符号 LE limbs）。
pub fn add_n(a: &[u64], b: &[u64]) -> Vec<u64> {
    let la = effective_len(a);
    let lb = effective_len(b);
    if la == 0 {
        return normalize(b);
    }
    if lb == 0 {
        return normalize(a);
    }
    let n = la.max(lb);
    let mut out = vec![0u64; n + 1];
    let mut carry = 0u64;
    for i in 0..n {
        let ai = if i < la { a[i] } else { 0 };
        let bi = if i < lb { b[i] } else { 0 };
        let (s0, c0) = ai.overflowing_add(bi);
        let (s1, c1) = s0.overflowing_add(carry);
        out[i] = s1;
        carry = u64::from(c0) + u64::from(c1);
    }
    out[n] = carry;
    normalize(&out)
}

/// `out = a - b`（要求 `a >= b`）。
///
/// # Panics
///
/// 若 `a < b`（oracle 合同：调用方保证不欠位）。
pub fn sub_n(a: &[u64], b: &[u64]) -> Vec<u64> {
    debug_assert!(cmp_slice(a, b) != core::cmp::Ordering::Less);
    let la = effective_len(a);
    let lb = effective_len(b);
    if lb == 0 {
        return normalize(a);
    }
    let n = la.max(lb);
    let mut out = vec![0u64; n];
    let mut borrow = 0u64;
    for i in 0..n {
        let ai = if i < la { a[i] } else { 0 };
        let bi = if i < lb { b[i] } else { 0 };
        let (d0, b0) = ai.overflowing_sub(bi);
        let (d1, b1) = d0.overflowing_sub(borrow);
        out[i] = d1;
        borrow = u64::from(b0) + u64::from(b1);
    }
    debug_assert_eq!(borrow, 0, "mpn_oracle::sub_n underflow");
    normalize(&out)
}

/// 无符号比较。
pub fn cmp_slice(a: &[u64], b: &[u64]) -> core::cmp::Ordering {
    let la = effective_len(a);
    let lb = effective_len(b);
    match la.cmp(&lb) {
        core::cmp::Ordering::Equal => {}
        other => return other,
    }
    for i in (0..la).rev() {
        match a[i].cmp(&b[i]) {
            core::cmp::Ordering::Equal => {}
            other => return other,
        }
    }
    core::cmp::Ordering::Equal
}

/// Schoolbook `out = a * b`。
pub fn mul_n(a: &[u64], b: &[u64]) -> Vec<u64> {
    let la = effective_len(a);
    let lb = effective_len(b);
    if la == 0 || lb == 0 {
        return vec![0];
    }
    let mut out = vec![0u64; la + lb];
    for i in 0..la {
        let mut carry = 0u128;
        for j in 0..lb {
            let acc = out[i + j] as u128 + (a[i] as u128) * (b[j] as u128) + carry;
            out[i + j] = acc as u64;
            carry = acc >> 64;
        }
        out[i + lb] = carry as u64;
    }
    normalize(&out)
}

/// `out = a * n`（单 limb 乘数）。
pub fn mul_1(a: &[u64], n: u64) -> Vec<u64> {
    let la = effective_len(a);
    if la == 0 || n == 0 {
        return vec![0];
    }
    let mut out = vec![0u64; la + 1];
    let mut carry = 0u128;
    for i in 0..la {
        let prod = (a[i] as u128) * (n as u128) + carry;
        out[i] = prod as u64;
        carry = prod >> 64;
    }
    out[la] = carry as u64;
    normalize(&out)
}

/// `out = a²`。
pub fn sqr(a: &[u64]) -> Vec<u64> {
    mul_n(a, a)
}

/// `(q, r) = div_rem(u, v)`，`v != 0`。Knuth D 的朴素单 limb / 多 limb 实现。
///
/// # Panics
///
/// `v` 有效长度为 0。
pub fn div_rem(u: &[u64], v: &[u64]) -> (Vec<u64>, Vec<u64>) {
    let lv = effective_len(v);
    assert!(lv > 0, "mpn_oracle::div_rem division by zero");
    let lu = effective_len(u);
    if lu == 0 {
        return (vec![0], vec![0]);
    }
    if cmp_slice(u, v) == core::cmp::Ordering::Less {
        return (vec![0], normalize(u));
    }
    if lv == 1 {
        return div_rem_1(u, v[0]);
    }

    // Normalize divisor so top bit of v[lv-1] is set.
    let shift = v[lv - 1].leading_zeros();
    let mut vn = vec![0u64; lv];
    let mut un = vec![0u64; lu + 1];
    shl_into(v, lv, shift, &mut vn);
    shl_into(u, lu, shift, &mut un);

    let mut q = vec![0u64; lu - lv + 1];
    for j in (0..=(lu - lv)).rev() {
        // Estimate qhat from two high limbs of remainder vs vn[lv-1].
        let uj_hi = un[j + lv];
        let uj_lo = un[j + lv - 1];
        let mut qhat = if uj_hi == vn[lv - 1] {
            u64::MAX
        }
        else {
            ((((uj_hi as u128) << 64) | (uj_lo as u128)) / (vn[lv - 1] as u128)) as u64
        };

        // Adjust while qhat * vn > un[j..j+lv+1] (at most twice).
        loop {
            let prod = mul_1(&vn, qhat);
            let window = &un[j..j + lv + 1];
            if cmp_slice(&prod, window) != core::cmp::Ordering::Greater {
                break;
            }
            qhat -= 1;
        }

        // un[j..] -= qhat * vn
        let prod = mul_1(&vn, qhat);
        let mut borrow = 0u64;
        let pn = effective_len(&prod).max(lv);
        for i in 0..=lv {
            let pi = if i < pn { prod.get(i).copied().unwrap_or(0) } else { 0 };
            let (d0, b0) = un[j + i].overflowing_sub(pi);
            let (d1, b1) = d0.overflowing_sub(borrow);
            un[j + i] = d1;
            borrow = u64::from(b0) + u64::from(b1);
        }
        if borrow != 0 {
            // Add back (should be rare after adjustment).
            let mut carry = 0u64;
            for i in 0..lv {
                let (s0, c0) = un[j + i].overflowing_add(vn[i]);
                let (s1, c1) = s0.overflowing_add(carry);
                un[j + i] = s1;
                carry = u64::from(c0) + u64::from(c1);
            }
            un[j + lv] = un[j + lv].wrapping_add(carry);
            qhat -= 1;
        }
        q[j] = qhat;
    }

    let rem = shr_normalize(&un, shift);
    (normalize(&q), rem)
}

fn div_rem_1(u: &[u64], d: u64) -> (Vec<u64>, Vec<u64>) {
    assert_ne!(d, 0);
    let lu = effective_len(u);
    if lu == 0 {
        return (vec![0], vec![0]);
    }
    let mut q = vec![0u64; lu];
    let mut rem = 0u128;
    for i in (0..lu).rev() {
        let cur = (rem << 64) | (u[i] as u128);
        q[i] = (cur / (d as u128)) as u64;
        rem = cur % (d as u128);
    }
    (normalize(&q), vec![rem as u64])
}

fn shl_into(src: &[u64], len: usize, shift: u32, dst: &mut [u64]) {
    if shift == 0 {
        dst[..len].copy_from_slice(&src[..len]);
        if dst.len() > len {
            dst[len] = 0;
        }
        return;
    }
    let mut carry = 0u64;
    for i in 0..len {
        let cur = src[i];
        dst[i] = (cur << shift) | carry;
        carry = cur >> (64 - shift);
    }
    if dst.len() > len {
        dst[len] = carry;
    }
}

fn shr_normalize(src: &[u64], shift: u32) -> Vec<u64> {
    let n = effective_len(src);
    if n == 0 {
        return vec![0];
    }
    if shift == 0 {
        return normalize(&src[..n]);
    }
    let mut out = vec![0u64; n];
    let mut carry = 0u64;
    for i in (0..n).rev() {
        let cur = src[i];
        out[i] = (cur >> shift) | carry;
        carry = cur << (64 - shift);
    }
    normalize(&out)
}

/// 与 [`is_zero`] 对称的公开零判定。
pub fn limbs_is_zero(limbs: &[u64]) -> bool {
    is_zero(limbs)
}
