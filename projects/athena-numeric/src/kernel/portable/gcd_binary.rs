//! # 用途
//! 二进制（Stein）GCD，无除法。
//!
//! # 数学模型
//! `gcd(2a,2b)=2gcd(a,b)`，对奇数 `b` 有 `gcd(2a,b)=gcd(a,b)`，
//! 对正奇数有 `gcd(a,b)=gcd(|a-b|,min(a,b))`。
//!
//! # 推导
//! 提出公有的 2 的幂，再对奇数值做减法并剥离新出现的 2 的因子，
//! 直至相等；最后恢复公共移位。
//!
//! # 算法步骤
//! 1. 处理零；记录 `min(v₂(a),v₂(b))`。
//! 2. 两端变奇；循环减法 / 交换 / 移位直至相等。
//! 3. 左移恢复保存的公共 2-进位值。
//!
//! # 前置条件
//! - 规范的非负量级。
//!
//! # 后置条件
//! - 规范的 `gcd`。
//!
//! # 复杂度
//! 当操作数共享小的 2 的因子时位复杂度有利；对宽随机奇数
//! 可能慢于 Lehmer。
//!
//! # 交叉阈值
//! 作为 gcd 的收尾路径，以及较小宽度时使用。
//!
//! # 失败模式
//! 除空/零处理外无额外失败。
//!
//! # 测试
//! `tests/exact/` 与 `tests/runtime/differential_pure.rs` 中的 Euclid 参考套件。

use std::cmp::Ordering;

use super::{
    convenience::sub_n,
    primitive::{cmp_slice, is_one, is_zero, normalize_trim, trailing_zeros},
    shift::{shl_assign, shr_assign, shr_assign_until_odd},
};

/// 二进制 GCD（Stein 算法）。
///
/// 去掉公有的 2 的幂，再反复从较大奇数值减去较小者，
/// 并剥离新露出的 2 的因子。有序正值上减法保持 gcd，
/// 移位只去掉 2 的幂。末尾恢复保存的公共移位。
/// 避免除法，但对宽随机输入可能不如 Lehmer。输入为规范量级。
pub(crate) fn binary_gcd(mut a: Vec<u64>, mut b: Vec<u64>) -> Vec<u64> {
    a = normalize_trim(a);
    b = normalize_trim(b);
    if is_zero(&a) {
        return b;
    }
    if is_zero(&b) {
        return a;
    }

    let shift = trailing_zeros(&a).min(trailing_zeros(&b));
    shr_assign(&mut a, shift);
    shr_assign(&mut b, shift);

    loop {
        shr_assign_until_odd(&mut a);
        shr_assign_until_odd(&mut b);
        if cmp_slice(&a, &b) == Ordering::Equal {
            break;
        }
        if cmp_slice(&a, &b) == Ordering::Less {
            std::mem::swap(&mut a, &mut b);
        }
        if is_one(&b) {
            break;
        }
        a = sub_n(&a, &b);
        shr_assign(&mut a, 1);
    }
    a = b;
    shl_assign(&mut a, shift);
    normalize_trim(a)
}
