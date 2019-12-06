use athena_ir::{ExtensionRegistry, SymbolTable};

#[test]
fn intern_stable() {
    let mut t = SymbolTable::new();
    let a = t.intern("x");
    let b = t.intern("x");
    assert_eq!(a, b);
    assert_eq!(t.resolve(a), Some("x"));
}

#[test]
fn extension_registry_starts_empty_without_surface_catalog() {
    let mut registry = ExtensionRegistry::new();
    assert!(registry.is_empty());
    // Display-name reverse lookup is not a public core API. Intern is the only
    // allocate path; identity is `ExtensionOperatorId`, not a Mathematica catalog.
    let plus = registry.intern("user_extension_plus");
    let again = registry.intern("user_extension_plus");
    assert_eq!(plus, again);
    assert_eq!(registry.display_name(plus), Some("user_extension_plus"));
    assert_eq!(registry.len(), 1);
}
