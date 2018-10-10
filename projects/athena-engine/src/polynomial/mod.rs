//! 多项式代数 — 环上的稀疏多项式（骨架）。
//!
//! 重型算法只在本模块；`athena-rewriter` 仅做轻量规范化。
//! 禁止 `HashMap<String, Number>` 作为长期表示。

mod algorithms;
mod builder;
mod canonical;
mod expr;
mod factor;
mod groebner;
mod hash;
mod operations;
mod order;
mod request;
mod result;
mod ring;
mod ring_table;
mod value;

pub use builder::{CanonicalPolynomial, PolynomialBuilder};
pub use canonical::canonicalize_polynomial;
pub use expr::{MonomialTerm, Polynomial};
pub use hash::canonical_hash as polynomial_canonical_hash;
pub use order::MonomialOrder;
pub use request::PolynomialRequest;
pub use result::{PolynomialResult, execute_polynomial, execute_polynomial_with_rings};
pub use ring::{CoefficientDomain, DivisionPolicy, RingCharacteristic, RingDescriptor};
pub use ring_table::RingTable;
pub use value::{PolynomialDomainValue, PolynomialValue};

pub use athena_types::RingId;
