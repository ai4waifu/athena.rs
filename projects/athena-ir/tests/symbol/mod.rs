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
    assert!(registry.lookup_display_name("Plus").is_none());
    assert!(registry.lookup_display_name("Blank").is_none());
    let deferred_define = format!("{}{}", "Define", "Deferred");
    assert!(registry.lookup_display_name(&deferred_define).is_none());
    let plus = registry.intern("Plus");
    assert_eq!(registry.lookup_display_name("Plus"), Some(plus));
    assert_eq!(registry.display_name(plus), Some("Plus"));
    assert_eq!(registry.len(), 1);
}
