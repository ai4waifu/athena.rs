//! 模整数（numeric 层值类型）。

use athena_types::Modulus;

use crate::integer::Integer;

/// 绑定模数的剩余类。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModularValue {
    /// 剩余。
    pub residue: Integer,
    /// 模数。
    pub modulus: Modulus,
}
