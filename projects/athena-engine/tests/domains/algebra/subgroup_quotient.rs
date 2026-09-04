//! 子群、同态与商群。

use athena_engine::domains::{
    algebra::{AlgebraMapKind, GroupTable},
    group::{
        GroupDomainValue, GroupElementRepr, GroupRequest, GroupResult, Permutation, apply_group_homomorphism,
        canonical_permutation, execute_group_with_table_mut, group_membership, project_quotient_element,
    },
};
use athena_numeric::Integer;
use athena_types::{DiagnosticCode, GroupElementId};

fn transposition_01(n: u32) -> Permutation {
    let mut images: Vec<u32> = (0..n).collect();
    images.swap(0, 1);
    Permutation { images }
}

fn cycle_012() -> Permutation {
    Permutation { images: vec![1, 2, 0] }
}

fn klein_four_gens() -> [Permutation; 2] {
    [Permutation { images: vec![1, 0, 3, 2] }, Permutation { images: vec![2, 3, 0, 1] }]
}

#[test]
fn subgroup_from_generators_and_membership() {
    let mut table = GroupTable::new();
    let k4 = table.permutation_group(4, &klein_four_gens()).unwrap();
    let subgroup_id = table.subgroup_from_generators(k4, &[klein_four_gens()[0].clone()]).unwrap();
    let sub = table.subgroup_record(subgroup_id).unwrap();
    assert_eq!(table.order(sub.group).unwrap(), Integer::from_i64(2));
    let h = canonical_permutation(&table, sub.group, klein_four_gens()[0].images.clone(), GroupElementId(1)).unwrap();
    assert!(group_membership(&table, sub.group, &h).unwrap());
}

#[test]
fn klein_four_subgroup_is_normal_and_quotient_order_two() {
    let mut table = GroupTable::new();
    let k4 = table.permutation_group(4, &klein_four_gens()).unwrap();
    let subgroup_id = table.subgroup_from_generators(k4, &[klein_four_gens()[0].clone()]).unwrap();
    assert!(table.is_normal_subgroup(subgroup_id).unwrap());
    let q = table.quotient_group(subgroup_id).unwrap();
    assert_eq!(table.order(q).unwrap(), Integer::from_i64(2));
}

#[test]
fn s3_alt_subgroup_normal_quotient() {
    let mut table = GroupTable::new();
    let s3 = table.permutation_group(3, &[transposition_01(3), cycle_012()]).unwrap();
    let a3 = table.subgroup_from_generators(s3, &[cycle_012()]).unwrap();
    assert_eq!(table.order(table.subgroup_record(a3).unwrap().group).unwrap(), Integer::from_i64(3));
    assert!(table.is_normal_subgroup(a3).unwrap());
    let q = table.quotient_group(a3).unwrap();
    assert_eq!(table.order(q).unwrap(), Integer::from_i64(2));
}

#[test]
fn non_normal_subgroup_quotient_rejected() {
    let mut table = GroupTable::new();
    let s3 = table.permutation_group(3, &[transposition_01(3), cycle_012()]).unwrap();
    let h = table.subgroup_from_generators(s3, &[transposition_01(3)]).unwrap();
    assert!(!table.is_normal_subgroup(h).unwrap());
    let err = table.quotient_group(h).unwrap_err();
    assert_eq!(err.code.as_str(), DiagnosticCode::GroupNotNormal.as_str());
}

#[test]
fn sign_homomorphism_s3_to_z2() {
    let mut table = GroupTable::new();
    let s3 = table.permutation_group(3, &[transposition_01(3), cycle_012()]).unwrap();
    let z2 = table.permutation_group(2, &[transposition_01(2)]).unwrap();
    let map =
        table.homomorphism_from_generator_images(s3, z2, &[transposition_01(2), Permutation { images: vec![0, 1] }]).unwrap();
    let s = canonical_permutation(&table, s3, transposition_01(3).images, GroupElementId(1)).unwrap();
    let c = canonical_permutation(&table, s3, cycle_012().images, GroupElementId(2)).unwrap();
    let fs = apply_group_homomorphism(&table, map, &s).unwrap();
    let fc = apply_group_homomorphism(&table, map, &c).unwrap();
    assert_eq!(fs.repr, GroupElementRepr::Permutation(transposition_01(2)));
    assert_eq!(fc.repr, GroupElementRepr::Permutation(Permutation { images: vec![0, 1] }));
}

#[test]
fn invalid_homomorphism_rejected() {
    let mut table = GroupTable::new();
    let s3 = table.permutation_group(3, &[transposition_01(3), cycle_012()]).unwrap();
    let z2 = table.permutation_group(2, &[transposition_01(2)]).unwrap();
    let err = table
        .homomorphism_from_generator_images(s3, z2, &[Permutation { images: vec![0, 1] }, transposition_01(2)])
        .unwrap_err();
    assert_eq!(err.code.as_str(), DiagnosticCode::GroupElementInvalid.as_str());
}

#[test]
fn quotient_projection_via_request() {
    let mut table = GroupTable::new();
    let s3 = table.permutation_group(3, &[transposition_01(3), cycle_012()]).unwrap();
    let req = GroupRequest::SubgroupFromGenerators { parent: s3, generators: vec![cycle_012()] };
    let sub = match execute_group_with_table_mut(req, &mut table) {
        GroupResult::Exact { value: GroupDomainValue::Subgroup(s) } => s,
        other => panic!("unexpected {other:?}"),
    };
    let q_req = GroupRequest::QuotientGroup { subgroup: sub.id };
    let q = match execute_group_with_table_mut(q_req, &mut table) {
        GroupResult::Exact { value: GroupDomainValue::Group(g) } => g.id,
        other => panic!("unexpected {other:?}"),
    };
    assert_eq!(table.order(q).unwrap(), Integer::from_i64(2));
    let t = canonical_permutation(&table, s3, transposition_01(3).images, GroupElementId(3)).unwrap();
    let proj = project_quotient_element(&table, sub.id, &t).unwrap();
    assert!(group_membership(&table, q, &proj).unwrap());
}

#[test]
fn subgroup_inclusion_map_registered() {
    let mut table = GroupTable::new();
    let s3 = table.permutation_group(3, &[transposition_01(3), cycle_012()]).unwrap();
    let h = table.subgroup_from_generators(s3, &[cycle_012()]).unwrap();
    let record = table.subgroup_record(h).unwrap();
    let map = table.map_table().get(record.inclusion).expect("inclusion map");
    assert_eq!(map.kind, AlgebraMapKind::SubgroupInclusion);
}
