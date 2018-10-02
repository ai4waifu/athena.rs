//! Built-in function registry smoke tests.

use athena_engine::lookup_function;

#[test]
fn registry_contains_gate7_names() {
    for name in ["Exp", "Sin", "Sinh", "ArcTan", "Gamma", "Erf", "Abs", "Sign"] {
        assert!(lookup_function(name).is_some(), "missing {name}");
    }
}
