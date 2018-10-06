//! 多项式代数 — 环上的稀疏多项式（骨架）。
//!
//! 重型算法只在本模块；`athena-rewriter` 仅做轻量规范化。
//! 禁止 `HashMap<String, Number>` 作为长期表示。

mod algorithms;
mod expr;
mod factor;
mod groebner;
mod operations;
mod order;
mod request;
mod result;
mod ring;
mod ring_table;
mod value;

pub use expr::{MonomialTerm, Polynomial};
pub use order::MonomialOrder;
pub use request::PolynomialRequest;
pub use result::{PolynomialResult, execute_polynomial};
pub use ring::{CoefficientDomain, DivisionPolicy, RingCharacteristic, RingDescriptor};
pub use ring_table::RingTable;
pub use value::{PolynomialDomainValue, PolynomialValue};

pub use athena_types::RingId;
