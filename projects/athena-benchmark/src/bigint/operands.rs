//! 统一输入生成（解析在热路径外）。

use athena_numeric::Integer;

use super::BigIntOp;

/// 十进制操作数对（及可选积，供 div）。
#[derive(Debug, Clone)]
pub struct OperandStrings {
    /// 左操作数。
    pub a: String,
    /// 右操作数。
    pub b: String,
}

/// 幂指数（按位宽）。
#[derive(Debug, Clone, Copy)]
pub struct PowExp {
    /// `u32` 指数。
    pub exp: u32,
}

/// 按位宽生成稳定十进制操作数（正整数）。
pub fn operand_strings(bits: u32) -> OperandStrings {
    let a = gen_decimal(bits, 0xC0FF_EE00_D15E_A5EDu64.wrapping_add(u64::from(bits)));
    let b = gen_decimal(bits, 0xDEAD_BEEF_F00D_CAFEu64.wrapping_add(u64::from(bits) * 17));
    OperandStrings { a, b }
}

/// 幂指数表（与历史 Criterion 矩阵一致）。
pub fn pow_exp(bits: u32) -> PowExp {
    let exp = match bits {
        64 => 17,
        256 => 9,
        1024 => 5,
        4096 => 2,
        // 更大位宽：指数保持 2，避免结果爆炸
        _ => 2,
    };
    PowExp { exp }
}

/// 操作是否需要积（div）。
pub fn needs_product(op: BigIntOp) -> bool {
    matches!(op, BigIntOp::Div)
}

fn gen_decimal(bits: u32, seed: u64) -> String {
    let mut n = Integer::from_u64(seed | 1);
    let mix = Integer::from_u64(0x9E37_79B9_7F4A_7C15);
    while n.bits() < u64::from(bits) {
        n = n.mul(&mix).add(&Integer::from_u64(0xD1B5_4A32_D192_ED03));
    }
    while n.bits() > u64::from(bits) {
        n = n.div(&Integer::from_i64(2)).expect("shrink bits");
    }
    if n.is_zero() {
        n = Integer::one();
    }
    n.to_decimal_string()
}
