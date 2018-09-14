//! p-adic 骨架。

use crate::integer::Integer;

/// p-adic 值。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PAdicValue {
    /// 素数。
    pub prime: Integer,
    /// 精度。
    pub precision: u32,
    /// 数字展开占位。
    pub digits: Vec<u32>,
}
