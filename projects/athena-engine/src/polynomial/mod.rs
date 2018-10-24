//! 多项式代数 — 环上的稀疏多项式（骨架）。
//!
//! 重型算法只在本模块；`athena-rewriter` 仅做轻量规范化。
//! 禁止 `HashMap<String, Number>` 作为长期表示。

mod certificate;
mod cache_key;
mod algorithms;
mod builder;
mod canonical;
mod coeff_kernel;
mod expr;
mod factor;
mod groebner;
mod hash;
mod ideal;
mod jit_gate;
mod mgraph;
mod operations;
mod order;
mod repr;
mod request;
mod result;
mod ring;
mod ring_table;
mod value;

pub use builder::{CanonicalPolynomial, PolynomialBuilder};
pub use cache_key::{PolynomialCacheKey, PolynomialCacheOp, cache_key_for_request};
pub use jit_gate::{JitParityOutcome, mul_with_jit_parity, parity_diagnostic};
pub use mgraph::{execute_polynomial_mgraph, record_polynomial_result};
pub use canonical::canonicalize_polynomial;
pub use certificate::{GroebnerAlgorithm, GroebnerCertificate};
pub use expr::{MonomialTerm, Polynomial};
pub use groebner::{GroebnerBasis, GroebnerLimits, compute_elimination_basis, compute_groebner_basis, reduce_ideal};
pub use hash::canonical_hash as polynomial_canonical_hash;
pub use ideal::Ideal;
pub use order::MonomialOrder;
pub use operations::{add_polynomial, mul_polynomial, sub_polynomial};
pub use repr::{PolynomialRepr, PolynomialReprBody, ReprTarget, reprs_mathematically_equal};
pub use request::PolynomialRequest;
pub use result::{PolynomialResult, execute_polynomial, execute_polynomial_with_rings};
pub use ring::{CoefficientDomain, DivisionPolicy, RingCharacteristic, RingDescriptor};
pub use ring_table::RingTable;
pub use value::{GroebnerBasisValue, PolynomialDomainValue, PolynomialValue};

pub use athena_types::RingId;
