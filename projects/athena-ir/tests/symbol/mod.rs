use athena_ir::SymbolTable;

#[test]
fn intern_stable() {
    let mut t = SymbolTable::new();
    let a = t.intern("x");
    let b = t.intern("x");
    assert_eq!(a, b);
    assert_eq!(t.resolve(a), Some("x"));
}
