//! 过渡期宿主 wire（十进制字符串）；**不是**执行态数值真相源。
//!
//! 执行 / IR / engine 一律使用 `athena_numeric::NumericValue`。

mod number;

pub use number::{ExactNumber, Number as WireNumber, RealNumber};
