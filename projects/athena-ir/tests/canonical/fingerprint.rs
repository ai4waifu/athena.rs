use athena_ir::{
    ApplicationHead, Atom, ExtensionRegistry, SemanticOperator, TermBuilder, TermNode, TermStore, canonical_hash, canonical_hash_named,
    fnv1a64,
};
use athena_types::SourceSpan;

const SPAN: SourceSpan = SourceSpan { start: 0, end: 0 };

fn build_add_x_y() -> (TermStore, athena_types::TermId) {
    let mut arena = TermStore::new();
    let mut b = TermBuilder::new(&mut arena);
    let x = b.symbol("x", SPAN);
    let y = b.symbol("y", SPAN);
    let plus = b.application_semantic(SemanticOperator::Add, vec![x, y], SPAN);
    (arena, plus)
}

#[test]
fn fnv1a64_basis_and_prime() {
    assert_eq!(fnv1a64(b""), 0xcbf2_9ce4_8422_2325);
    assert_eq!(fnv1a64(b"a"), 0xaf63_dc4c_8601_ec8c);
}

#[test]
fn canonical_hash_order_independent() {
    let (a1, r1) = build_add_x_y();
    let (a2, r2) = build_add_x_y();
    assert_eq!(canonical_hash(&a1, r1), canonical_hash(&a2, r2));
}

#[test]
fn canonical_hash_semantic_stable_without_registry() {
    let (a1, r1) = build_add_x_y();
    let (a2, r2) = build_add_x_y();
    assert_eq!(canonical_hash(&a1, r1), canonical_hash(&a2, r2));
    assert_eq!(
        canonical_hash_named(&a1, &ExtensionRegistry::new(), r1),
        canonical_hash(&a1, r1)
    );
}

#[test]
fn canonical_hash_named_extension_registry_order_independent() {
    let mut reg_a = ExtensionRegistry::new();
    reg_a.intern("Sin");
    let plus_a = reg_a.intern("Foo");

    let mut reg_b = ExtensionRegistry::new();
    let plus_b = reg_b.intern("Foo");
    reg_b.intern("Sin");

    let mut arena_a = TermStore::new();
    let mut b_a = TermBuilder::new(&mut arena_a);
    let one_a = b_a.int(1, SPAN);
    let two_a = b_a.int(2, SPAN);
    let t_a = b_a.application(ApplicationHead::Extension(plus_a), vec![one_a, two_a], SPAN);

    let mut arena_b = TermStore::new();
    let mut b_b = TermBuilder::new(&mut arena_b);
    let one_b = b_b.int(1, SPAN);
    let two_b = b_b.int(2, SPAN);
    let t_b = b_b.application(ApplicationHead::Extension(plus_b), vec![one_b, two_b], SPAN);

    assert_eq!(canonical_hash_named(&arena_a, &reg_a, t_a), canonical_hash_named(&arena_b, &reg_b, t_b));
    // Without registry, extension ids differ across registries → hash differs.
    assert_ne!(canonical_hash(&arena_a, t_a), canonical_hash(&arena_b, t_b));
}

#[test]
fn structural_eq_value_and_structure() {
    let (arena, r1) = build_add_x_y();
    let (arena2, r2) = build_add_x_y();
    assert!(arena.structural_eq(r1, r2));

    let mut arena3 = TermStore::new();
    let mut b = TermBuilder::new(&mut arena3);
    let x = b.symbol("x", SPAN);
    let plus = b.application_semantic(SemanticOperator::Add, vec![x, x], SPAN);
    assert!(!arena3.structural_eq(plus, r1));
    assert!(arena3.structural_eq(plus, plus));
}

#[test]
fn structural_eq_atom_payloads() {
    let mut arena = TermStore::new();
    let mut b = TermBuilder::new(&mut arena);
    let i1 = b.int(2, SPAN);
    let i2 = b.int(2, SPAN);
    let r = b.rational_i64(1, 2, SPAN).unwrap();
    let f = b.real(1.5, SPAN);
    let tr = b.boolean(true, SPAN);
    assert!(arena.structural_eq(i1, i2));
    assert!(!arena.structural_eq(i1, r));
    assert!(!arena.structural_eq(i1, f));
    assert!(!arena.structural_eq(tr, i1));
    assert!(matches!(arena.get(i1), Some(TermNode::Atom(Atom::Number(_)))));
}
