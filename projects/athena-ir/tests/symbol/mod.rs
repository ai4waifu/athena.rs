use athena_ir::{OperatorRegistry, SymbolTable};

#[test]
fn intern_stable() {
    let mut t = SymbolTable::new();
    let a = t.intern("x");
    let b = t.intern("x");
    assert_eq!(a, b);
    assert_eq!(t.resolve(a), Some("x"));
}

#[test]
fn operator_registry_starts_empty_without_surface_catalog() {
    let mut registry = OperatorRegistry::new();
    assert!(registry.is_empty());
    assert!(registry.lookup("Plus").is_none());
    assert!(registry.lookup("SetDelayed").is_none());
    assert!(registry.lookup("Blank").is_none());
    let plus = registry.intern("Plus");
    assert_eq!(registry.lookup("Plus"), Some(plus));
    assert_eq!(registry.name(plus), Some("Plus"));
    assert_eq!(registry.len(), 1);
}
