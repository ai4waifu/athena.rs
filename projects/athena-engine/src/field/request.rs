//! 域论域请求。

use athena_numeric::Integer;
use athena_types::FieldId;

use super::types::FieldElement;

/// 域论域请求（骨架）。
#[derive(Debug, PartialEq, Eq)]
pub enum FieldRequest {
    /// 素域 𝔽_p。
    PrimeField {
        /// 素数 p。
        characteristic: Integer,
    },
    /// 有理数域。
    Rationals,
    /// 元素加法。
    Add {
        /// 左。
        lhs: FieldElement,
        /// 右。
        rhs: FieldElement,
    },
    /// 元素乘法。
    Mul {
        /// 左。
        lhs: FieldElement,
        /// 右。
        rhs: FieldElement,
    },
    /// 逆元。
    Inverse {
        /// 元素。
        element: FieldElement,
    },
    /// 查询域对象（Session 占位）。
    Lookup {
        /// 域 id。
        field: FieldId,
    },
}
