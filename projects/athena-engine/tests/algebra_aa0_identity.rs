//! AA0/AA1：fingerprint · map verification · 抽象 descriptor 门禁。

use athena_engine::{
    FieldFingerprint, FieldTable, GroupFingerprint, GroupRequest, GroupTable, Integer, MapVerification, MapVerificationKind,
    Permutation, PropertyWitness, execute_group_with_table_mut,
};

#[test]
fn field_fingerprint_stable_across_sessions() {
    let mut a = FieldTable::new();
    let mut b = FieldTable::new();
    let fa = a.prime_field(Integer::from_i64(7)).unwrap();
    let fb = b.prime_field(Integer::from_i64(7)).unwrap();
    assert_eq!(fa.0, fb.0); // both start from 0 in fresh tables
    assert_eq!(a.field_fingerprint(fa), b.field_fingerprint(fb));
    assert_eq!(a.field_fingerprint(fa), Some(FieldFingerprint::prime_field(&Integer::from_i64(7))));
    let qa = a.rationals();
    let qb = b.rationals();
    assert_ne!(a.field_fingerprint(qa), a.field_fingerprint(fa));
    assert_eq!(a.field_fingerprint(qa), b.field_fingerprint(qb));
}

#[test]
fn group_fingerprint_matches_generator_content() {
    let mut table = GroupTable::new();
    let gens = vec![Permutation { images: vec![1, 0, 2] }];
    let g = table.permutation_group(3, &gens).unwrap();
    let fp = table.group_fingerprint(g).unwrap();
    assert_eq!(fp, GroupFingerprint::from_permutation_generators(3, &[vec![1, 0, 2]]));
    let g2 = table.permutation_group(3, &gens).unwrap();
    assert_eq!(g, g2);
    assert_eq!(table.group_fingerprint(g2), Some(fp));
}

#[test]
fn map_verification_has_no_thin_bool() {
    let v = MapVerification::proven(MapVerificationKind::DegreeCheck, PropertyWitness::placeholder("degree_check"));
    assert!(v.is_proven());
    assert!(!MapVerification::unverified().is_proven());
}

#[test]
fn permutation_group_record_is_computable_descriptor() {
    let mut table = GroupTable::new();
    let g = match execute_group_with_table_mut(
        GroupRequest::PermutationGroup { degree: 2, generators: vec![Permutation { images: vec![1, 0] }] },
        &mut table,
    ) {
        athena_engine::GroupResult::Exact { value: athena_engine::GroupDomainValue::Group(g) } => g,
        other => panic!("expected group, got {other:?}"),
    };
    assert!(matches!(g.descriptor, athena_engine::GroupDescriptor::Permutation { degree: 2 }));
    table.ensure_computable(g.id).unwrap();
}
