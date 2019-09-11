//! Built-in unary function registry smoke.

use athena_engine::execution::builtins::registry::lookup_unary;
use athena_ir::UnaryFunction;

#[test]
fn registry_contains_closed_unary_functions() {
    for f in [
        UnaryFunction::Exp,
        UnaryFunction::Sin,
        UnaryFunction::Sinh,
        UnaryFunction::ArcTan,
        UnaryFunction::Gamma,
        UnaryFunction::Erf,
        UnaryFunction::Abs,
        UnaryFunction::Sign,
    ] {
        assert!(lookup_unary(f).is_some(), "missing {f:?}");
    }
}
