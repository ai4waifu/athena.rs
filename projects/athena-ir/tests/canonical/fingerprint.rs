use athena_ir::{Atom, OperatorRegistry, TermBuilder, TermNode, TermStore, canonical_hash, canonical_hash_named, fnv1a64};
use athena_types::SourceSpan;

const SPAN: SourceSpan = SourceSpan { start: 0, end: 0 };

fn build_plus_x_plus_y(x_first: bool) -> (TermStore, athena_types::TermId) {
    let mut arena = TermStore::new();
    let mut b = TermBuilder::new(&mut arena);
    let x = b.symbol("x", SPAN);
    let y = b.symbol("y", SPAN);
    let plus = b.application_named(&mut OperatorRegistry::new(), "Plus", vec![x, y], SPAN);
    let _ = x_first;
    (arena, plus)
}

#[test]
fn fnv1a64_basis_and_prime() {
    assert_eq!(fnv1a64(b""), 0xcbf2_9ce4_8422_2325);
    assert_eq!(fnv1a64(b"a"), 0xaf63_dc4c_8601_ec8c);
}

#[test]
fn canonical_hash_order_independent() {
    // 同结构（不同插入顺序构造的两个 arena）同 hash。
    let (a1, r1) = build_plus_x_plus_y(true);
    let (a2, r2) = build_plus_x_plus_y(false);
    assert_eq!(canonical_hash(&a1, r1), canonical_hash(&a2, r2));
}

#[test]
fn canonical_hash_named_registry_order_independent() {
    let mut reg_a = OperatorRegistry::new();
    reg_a.intern("Sin");
    let plus_a = reg_a.intern("Plus");

    let mut reg_b = OperatorRegistry::new();
    let plus_b = reg_b.intern("Plus");
    reg_b.intern("Sin");

    let mut arena_a = TermStore::new();
    let mut b_a = TermBuilder::new(&mut arena_a);
    let one_a = b_a.int(1, SPAN);
    let two_a = b_a.int(2, SPAN);
    let t_a = b_a.application(plus_a, vec![one_a, two_a], SPAN);

    let mut arena_b = TermStore::new();
    let mut b_b = TermBuilder::new(&mut arena_b);
    let one_b = b_b.int(1, SPAN);
    let two_b = b_b.int(2, SPAN);
    let t_b = b_b.application(plus_b, vec![one_b, two_b], SPAN);

    assert_eq!(canonical_hash_named(&arena_a, &reg_a, t_a), canonical_hash_named(&arena_b, &reg_b, t_b));
    // 未提供注册表时退化为 op id，两注册表 id 不同则 hash 不同。
    assert_ne!(canonical_hash(&arena_a, t_a), canonical_hash(&arena_b, t_b));
}

#[test]
fn structural_eq_value_and_structure() {
    let (arena, r1) = build_plus_x_plus_y(true);
    let (arena2, r2) = build_plus_x_plus_y(false);
    assert!(arena.structural_eq(r1, r2));

    let mut arena3 = TermStore::new();
    let mut b = TermBuilder::new(&mut arena3);
    let x = b.symbol("x", SPAN);
    let plus = b.application_named(&mut OperatorRegistry::new(), "Plus", vec![x, x], SPAN);
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
