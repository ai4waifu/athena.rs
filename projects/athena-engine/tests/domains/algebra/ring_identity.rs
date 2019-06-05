//! 代数身份：指纹、映射验证、抽象描述子守卫。

use athena_engine::domains::{
    algebra::{FieldFingerprint, FieldTable, GroupFingerprint, GroupTable, MapVerification, MapVerificationKind, PropertyWitness},
    group::{GroupRequest, Permutation, execute_group_with_table_mut},
};
use athena_numeric::Integer;

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
fn abstract_group_id_rejected_by_ensure_computable() {
    let table = GroupTable::new();
    // 从未注册的新 `GroupId` → 不可计算。
    let err = table.ensure_computable(athena_types::GroupId(99)).unwrap_err();
    assert_eq!(err.details.get("operation").map(ToString::to_string).as_deref(), Some("abstract_descriptor_not_computable"));
}

#[test]
fn unproven_map_rejected_by_require_proven() {
    use athena_engine::domains::algebra::{AlgebraMap, AlgebraMapKind, AlgebraParentId};
    use athena_types::{AlgebraMapId, FieldId};
    let map = AlgebraMap {
        id: AlgebraMapId(0),
        source: AlgebraParentId::Field(FieldId(0)),
        target: AlgebraParentId::Field(FieldId(1)),
        kind: AlgebraMapKind::FieldEmbedding,
        verification: MapVerification::unverified(),
    };
    assert!(map.require_proven().is_err());
}

#[test]
fn permutation_group_record_is_computable_descriptor() {
    let mut table = GroupTable::new();
    let g = match execute_group_with_table_mut(
        GroupRequest::PermutationGroup { degree: 2, generators: vec![Permutation { images: vec![1, 0] }] },
        &mut table,
    ) {
        athena_engine::domains::group::GroupResult::Exact { value: athena_engine::domains::group::GroupDomainValue::Group(g) } => g,
        other => panic!("expected group, got {other:?}"),
    };
    assert!(matches!(g.descriptor, athena_engine::domains::group::GroupDescriptor::Permutation { degree: 2 }));
    table.ensure_computable(g.id).unwrap();
}
