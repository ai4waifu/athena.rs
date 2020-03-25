#![doc = include_str!("readme.md")]

//! 多项式代数 — 环上的稀疏多项式（骨架）。
//!
//! 重型算法只在本模块；`athena-rewriter` 仅做轻量规范化。
//! 禁止 `HashMap<String, Number>` 作为长期表示。

mod algorithms;
mod builder;
mod cache_key;
mod canonical;
mod certificate;
pub mod coefficient_kernel;
mod coefficient_ring_table;
mod exponent;
mod f4;
mod factor;
pub mod fingerprint;
mod groebner;
mod hash;
mod ideal;
mod jit_gate;
mod mgraph;
mod modular_image;
mod monomial_layout;
pub mod object;
mod object_ref;
mod operations;
mod order;
mod repr;
pub mod request;
mod result;
mod ring;
pub mod ring_table;
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
pub use f4::{
    F4CriticalPair, F4SymbolicRow, F4UpdateComputation, F4UpdateLimits, MacaulayCsrMatrix, MacaulayMatrix, MacaulayRowInput,
    build_macaulay_csr, build_macaulay_matrix, eliminate_macaulay_column, f4_matrix_reduce_pairs, macaulay_matrix_polynomials,
    macaulay_row_to_polynomial, pair_sugar_degree, pair_sugar_with, polynomial_sugar, reduce_macaulay_matrix, resume_f4_basis_update,
    run_f4_basis_update, select_minimal_sugar_pairs, select_minimal_sugar_pairs_with, symbolic_preprocess_closure, symbolic_preprocess_pairs,
};
pub use factor::{
    PolynomialCofactorStatus, PolynomialFactorComponent, PolynomialFactorLimits, PolynomialFactorStatus, PolynomialFactorization,
    PolynomialFactorizationCompleteness, factor_univariate,
};
pub use fingerprint::{
    FINGERPRINT_ALGORITHM, PolynomialFingerprint, RingFingerprint, RingHandle, fnv1a64, polynomial_fingerprint, polynomial_fingerprint_u64,
};
pub use groebner::{
    GroebnerComputation, GroebnerFrontier, GroebnerLimits, GroebnerVerificationReport, VerifiedGroebnerBasis, chain_criterion_applies,
    compute_elimination_basis, compute_groebner_basis, compute_groebner_basis_f4, ideal_membership, ordered_pair, reduce_by_verified,
    reduce_ideal, resume_groebner_basis, resume_groebner_basis_f4, verify_groebner_basis,
};
pub use hash::canonical_hash as polynomial_canonical_hash;
pub use ideal::Ideal;
pub use jit_gate::{JitParityOutcome, mul_with_jit_parity, parity_diagnostic};
pub use mgraph::{execute_polynomial_mgraph, record_polynomial_result};
pub use modular_image::{
    CrtPolynomialCombination, ModularImage, crt_combine_and_reconstruct, crt_combine_and_reconstruct_finite_field_polys,
    crt_combine_modular_images, map_generators_mod_prime, map_polynomial_mod_prime, modular_image_from_finite_field_poly,
    reconstruct_and_verify_groebner_basis_via_crt, reconstruct_groebner_basis_via_crt, reconstruct_polynomial_from_finite_field_ring,
    reconstruct_polynomial_from_modular_image, reconstruct_rational_coefficient,
};
pub use monomial_layout::{CompiledBlockSegment, CompiledMonomialOrder, MonomialLayout, PackedMonomial};
pub use object::{CanonicalPolynomial, MonomialTerm, Polynomial};
pub use object_ref::{PolynomialObjectStore, PolynomialRef, intern_request_object_refs, object_refs_for, refs_from_request};
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
