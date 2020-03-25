//! Owning 复制辅助（数值类型无 [`Clone`]）。
//!
//! 与 [`athena_numeric::Integer::add`] 同合同：经 [`NumericContext::portable_default`]，
//! 预算无界时 `expect`。有 Session / 显式 context 的路径应改调 [`Integer::try_clone_in`]。

use athena_numeric::{Integer, ModularValue, Modulus, Number, NumericContext, Rational, natural::Natural};
use athena_types::Result;

fn portable() -> NumericContext {
    NumericContext::portable_default()
}

/// [`Integer`] owning 复制（portable 无界 context）。
#[inline]
pub fn clone_integer(n: &Integer) -> Integer {
    n.try_clone_in(&portable()).expect("portable default max_limbs unbounded")
}

/// [`Natural`] owning 复制。
#[inline]
pub fn clone_natural(n: &Natural) -> Natural {
    n.try_clone_in(&portable()).expect("portable default max_limbs unbounded")
}

/// [`Rational`] owning 复制。
#[inline]
pub fn clone_rational(r: &Rational) -> Rational {
    r.try_clone_in(&portable()).expect("portable default max_limbs unbounded")
}

/// [`Number`] / [`NumericValue`] owning 复制。
#[inline]
pub fn clone_number(n: &Number) -> Number {
    n.try_clone_in(&portable()).expect("portable default max_limbs unbounded")
}

/// [`Modulus`] owning 复制。
#[inline]
pub fn clone_modulus(m: &Modulus) -> Modulus {
    m.try_clone_in(&portable()).expect("portable default max_limbs unbounded")
}

/// [`ModularValue`] owning 复制。
#[inline]
pub fn clone_modular(m: &ModularValue) -> ModularValue {
    m.try_clone_in(&portable()).expect("portable default max_limbs unbounded")
}

/// 可失败 [`Integer`] 复制（调用方已有 `ctx`）。
#[inline]
pub fn try_clone_integer(n: &Integer, ctx: &NumericContext) -> Result<Integer> {
    n.try_clone_in(ctx)
}

/// Vec<Integer> 扩容（不依赖 Integer: Clone）。
pub fn resize_integers(v: &mut Vec<Integer>, new_len: usize, fill: &Integer) {
    if new_len <= v.len() {
        v.truncate(new_len);
        return;
    }
    v.reserve(new_len - v.len());
    while v.len() < new_len {
        v.push(clone_integer(fill));
    }
}

/// Vec<Rational> 扩容。
pub fn resize_rationals(v: &mut Vec<Rational>, new_len: usize, fill: &Rational) {
    if new_len <= v.len() {
        v.truncate(new_len);
        return;
    }
    v.reserve(new_len - v.len());
    while v.len() < new_len {
        v.push(clone_rational(fill));
    }
}

/// Vec<Number> / Vec<NumericValue> 扩容。
pub fn resize_numbers(v: &mut Vec<Number>, new_len: usize, fill: &Number) {
    if new_len <= v.len() {
        v.truncate(new_len);
        return;
    }
    v.reserve(new_len - v.len());
    while v.len() < new_len {
        v.push(clone_number(fill));
    }
}

/// 克隆 `Vec<Integer>`。
pub fn clone_integers(v: &[Integer]) -> Vec<Integer> {
    v.iter().map(clone_integer).collect()
}

/// 克隆 `Vec<Rational>`。
pub fn clone_rationals(v: &[Rational]) -> Vec<Rational> {
    v.iter().map(clone_rational).collect()
}

/// 克隆 `Vec<Number>`。
pub fn clone_numbers(v: &[Number]) -> Vec<Number> {
    v.iter().map(clone_number).collect()
}
