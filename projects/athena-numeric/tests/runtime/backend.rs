//! 数值 backend 合同测试。

use athena_numeric::{
    NumericBackend, NumericCapability, NumericDomain, NumericOperation, NumericResultMode, PrecisionKind, PureRustBackend,
};

#[test]
fn pure_rust_contract_is_wasm_safe_and_deterministic() {
    let b = PureRustBackend;
    assert!(b.wasm_safe());
    assert!(b.has_capability(NumericCapability::ExactInteger));
    assert!(b.supports_domain(&NumericDomain::Integer));
    assert!(b.supports_precision(PrecisionKind::Exact));
    assert!(!b.contract().native_only);
}

#[test]
fn pure_rust_supports_integer_exact_add() {
    let b = PureRustBackend;
    assert!(b.supports_operation(&NumericDomain::Integer, NumericOperation::Add, NumericResultMode::Exact));
    assert!(!b.supports_operation(&NumericDomain::Integer, NumericOperation::Add, NumericResultMode::Certified));
}

#[test]
fn pure_rust_advertises_directed_interval_enclosure() {
    let b = PureRustBackend;
    assert!(b.has_capability(NumericCapability::DirectedRounding));
    assert!(b.supports_operation(
        &NumericDomain::Interval,
        NumericOperation::IntervalMul,
        NumericResultMode::IntervalEnclosure
    ));
}
