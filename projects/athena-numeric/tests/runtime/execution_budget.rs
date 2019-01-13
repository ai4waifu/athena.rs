//! 执行预算公开合同测试。

use athena_numeric::{Dyadic, ExecutionBudget, NumericBackendLimits, NumericContext};

#[test]
fn budget_rejects_excessive_limbs() {
    let budget = ExecutionBudget::from_limits(&NumericBackendLimits {
        max_limbs: Some(2),
        max_significand_bits: None,
        max_wire_payload_bytes: None,
        max_pow_exp: None,
    });
    let err = budget.check_mul(3, 3).unwrap_err();
    assert_eq!(err.code.as_str(), "ATHENA_NUMERIC_RESOURCE_LIMIT");
}

#[test]
fn numeric_context_pure_rust_default_has_wire_limit() {
    let ctx = NumericContext::pure_rust_default();
    assert!(ctx.budget().max_wire_payload_bytes().is_some());
}

#[test]
fn dyadic_f64_roundtrip_large() {
    let d = Dyadic::from_f64(1.5).unwrap();
    assert_eq!(d.to_f64_exact(), Some(1.5));
}
