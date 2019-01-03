//! Jacobi 与 Kronecker 符号。

use athena_numeric::Integer;

/// Jacobi 符号 `(a/n)`，`n` 须为正奇数。
pub fn jacobi_symbol(a: &Integer, n: &Integer) -> Option<i32> {
    if !n.is_positive() || n.rem(&Integer::from_i64(2)).expect("rem").is_zero() {
        return None;
    }
    Some(jacobi_inner(a.rem(n).expect("rem"), n.clone()))
}

/// Kronecker 符号 `(a/n)`。
pub fn kronecker_symbol(a: &Integer, n: &Integer) -> i32 {
    if n.is_zero() {
        return if a.abs().is_one() { 1 } else { 0 };
    }
    if n == &Integer::from_i64(-1) {
        return if a.is_negative() { -1 } else { 1 };
    }
    if n == &Integer::from_i64(2) {
        return kronecker_two(a);
    }
    let mut acc = 1i32;
    let mut aa = a.clone();
    let mut nn = n.clone();
    if nn.is_negative() {
        if aa.is_negative() {
            acc = -acc;
        }
        nn = nn.neg();
    }
    aa = aa.rem(&nn).expect("rem");
    if aa.is_negative() {
        aa = aa.add(&nn);
    }
    acc * jacobi_inner(aa, nn)
}

fn kronecker_two(a: &Integer) -> i32 {
    if a.is_zero() {
        return 0;
    }
    let r = a.rem(&Integer::from_i64(8)).expect("rem");
    let v = r.to_u64().unwrap_or(0);
    if v == 1 || v == 7 { 1 } else { -1 }
}

fn jacobi_inner(mut a: Integer, mut n: Integer) -> i32 {
    let mut result = 1i32;
    if n.is_one() {
        return 1;
    }
    if a.is_zero() {
        return 0;
    }
    a = a.rem(&n).expect("rem");
    if a.is_negative() {
        a = a.add(&n);
    }

    while !a.is_zero() {
        while a.rem(&Integer::from_i64(2)).expect("rem").is_zero() {
            a = a.div(&Integer::from_i64(2)).expect("div");
            let n8 = n.rem(&Integer::from_i64(8)).expect("rem").to_u64().unwrap_or(0);
            if n8 == 3 || n8 == 5 {
                result = -result;
            }
        }
        if a < n {
            std::mem::swap(&mut a, &mut n);
            let a8 = a.rem(&Integer::from_i64(8)).expect("rem").to_u64().unwrap_or(0);
            let n8 = n.rem(&Integer::from_i64(8)).expect("rem").to_u64().unwrap_or(0);
            if a8 == 3 && n8 == 3 {
                result = -result;
            }
        }
        a = a.sub(&n);
    }
    if n.is_one() { result } else { 0 }
}
