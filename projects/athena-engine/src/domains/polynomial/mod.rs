//! 多项式代数 — 环上的稀疏多项式（骨架）。
//!
//! 重型算法只在本模块；`athena-rewriter` 仅做轻量规范化。
//! 禁止 `HashMap<String, Number>` 作为长期表示。

mod algorithms;
mod builder;
mod cache_key;
mod canonical;
mod certificate;
mod coefficient_kernel;
mod coefficient_ring_table;
mod exponent;
mod factor;
mod fingerprint;
mod groebner;
mod hash;
mod ideal;
mod jit_gate;
mod mgraph;
mod monomial_layout;
mod object;
mod operations;
mod order;
mod repr;
mod request;
mod result;
mod ring;
mod ring_table;
mod univariate;
mod value;

pub use athena_types::CoefficientRingId;
pub use builder::PolynomialBuilder;
pub use cache_key::{PolynomialCacheKey, PolynomialCacheOp, cache_key_for_request};
pub use canonical::canonicalize_polynomial;
pub use certificate::{GroebnerAlgorithm, GroebnerCertificate, GroebnerStatus};
pub use coefficient_kernel::{
    CoefficientRing, FpBigKernel, FpWordKernel, QCoefficientKernel, SpecializedCoefficientKernel, ZCoefficientKernel,
};
pub use coefficient_ring_table::{CoefficientRingDescriptor, CoefficientRingTable};
pub use factor::{
    PolynomialCofactorStatus, PolynomialFactorComponent, PolynomialFactorLimits, PolynomialFactorStatus, PolynomialFactorization,
    PolynomialFactorizationCompleteness, factor_univariate,
};
pub use fingerprint::{
    FINGERPRINT_ALGORITHM, PolynomialFingerprint, RingFingerprint, RingHandle, fnv1a64, polynomial_fingerprint, polynomial_fingerprint_u64,
};
pub use groebner::{
    GroebnerComputation, GroebnerFrontier, GroebnerLimits, GroebnerVerificationReport, VerifiedGroebnerBasis,
    compute_elimination_basis, compute_groebner_basis, ideal_membership, reduce_by_verified, reduce_ideal, verify_groebner_basis,
};
pub use hash::canonical_hash as polynomial_canonical_hash;
pub use ideal::Ideal;
pub use jit_gate::{JitParityOutcome, mul_with_jit_parity, parity_diagnostic};
pub use mgraph::{execute_polynomial_mgraph, record_polynomial_result};
pub use monomial_layout::{CompiledBlockSegment, CompiledMonomialOrder, MonomialLayout, PackedMonomial};
pub use object::{MonomialTerm, Polynomial};
pub use operations::{add_polynomial, mul_polynomial, sub_polynomial};
pub use order::MonomialOrder;
pub use repr::{PolynomialRepr, PolynomialReprBody, ReprTarget, reprs_mathematically_equal};
pub use request::PolynomialRequest;
pub use result::{PolynomialResult, execute_polynomial, execute_polynomial_with_rings};
pub use ring::{CoefficientDomain, DivisionPolicy, RingCharacteristic, RingDescriptor};
pub use ring_table::RingTable;
pub use univariate::{UnivariateDivision, div_univariate, gcd_univariate, resultant_univariate};
pub use value::{GroebnerBasisValue, PolynomialDomainValue, PolynomialValue, UnivariateDivisionValue};

pub use athena_types::RingId;
