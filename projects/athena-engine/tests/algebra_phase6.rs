//! 代数父对象 Phase 6：置换 presentation + BSGS（Schreier–Sims）。

use athena_engine::{
    GroupElementId, GroupElementRepr, GroupRequest, GroupTable, Integer, Permutation, canonical_permutation,
    execute_group_with_table_mut, group_membership, inverse_group_element, multiply_group_elements,
};
use athena_types::DiagnosticCode;

fn transposition_01(n: u32) -> Permutation {
    let mut images: Vec<u32> = (0..n).collect();
    images.swap(0, 1);
    Permutation { images }
}

fn cycle_012() -> Permutation {
    Permutation { images: vec![1, 2, 0] }
}

#[test]
fn permutation_group_interns_and_computes_order() {
    let mut table = GroupTable::new();
    let g = table.permutation_group(3, &[transposition_01(3), cycle_012()]).unwrap();
    assert_eq!(table.order(g).unwrap(), Integer::from_i64(6));
}

#[test]
fn invalid_permutation_rejected() {
    let mut table = GroupTable::new();
    let bad = Permutation { images: vec![0, 0, 2] };
    let err = table.permutation_group(3, &[bad]).unwrap_err();
    assert_eq!(err.code.as_str(), DiagnosticCode::PermutationInvalid.as_str());
}

#[test]
fn compose_follows_p_q_i_convention() {
    let mut table = GroupTable::new();
    let g = table.permutation_group(3, &[transposition_01(3)]).unwrap();
    let p = canonical_permutation(&table, g, transposition_01(3).images, GroupElementId(1)).unwrap();
    let q = canonical_permutation(&table, g, cycle_012().images, GroupElementId(2)).unwrap();
    let pq = multiply_group_elements(&table, &p, &q).unwrap();
    let raw_p = transposition_01(3);
    let raw_q = cycle_012();
    let expected: Vec<u32> = (0..3).map(|i| {
        let qi = raw_q.images[i as usize];
        raw_p.images[qi as usize]
    }).collect();
    match pq.repr {
        GroupElementRepr::Permutation(perm) => assert_eq!(perm.images, expected),
        _ => panic!("expected permutation repr"),
    }
}

fn klein_generators() -> (Permutation, Permutation) {
    (
        Permutation { images: vec![1, 0, 3, 2] },
        Permutation { images: vec![2, 3, 0, 1] },
    )
}

#[test]
fn bsgs_membership_and_inverse() {
    let mut table = GroupTable::new();
    let (a, b) = klein_generators();
    let g = table.permutation_group(4, &[a.clone(), b]).unwrap();
    assert_eq!(table.order(g).unwrap(), Integer::from_i64(4));
    let x = canonical_permutation(&table, g, a.images, GroupElementId(1)).unwrap();
    assert!(group_membership(&table, g, &x).unwrap());
    let inv = inverse_group_element(&table, &x).unwrap();
    let prod = multiply_group_elements(&table, &x, &inv).unwrap();
    match prod.repr {
        GroupElementRepr::Permutation(p) => assert_eq!(p.images, (0..4).collect::<Vec<_>>()),
        _ => panic!("expected identity permutation"),
    }
}

#[test]
fn execute_group_registers_permutation_group() {
    let mut table = GroupTable::new();
    let r = execute_group_with_table_mut(
        GroupRequest::PermutationGroup { degree: 3, generators: vec![transposition_01(3), cycle_012()] },
        &mut table,
    );
    assert!(matches!(r, athena_engine::GroupResult::Exact { .. }));
}

#[test]
fn klein_four_is_abelian() {
    let mut table = GroupTable::new();
    let (a, b) = klein_generators();
    let g = table.permutation_group(4, &[a, b]).unwrap();
    let r = athena_engine::execute_group_with_table(
        GroupRequest::IsAbelian { group: g },
        &table,
    );
    match r {
        athena_engine::GroupResult::Exact { value } => {
            assert!(matches!(value, athena_engine::GroupDomainValue::Boolean(true)));
        }
        _ => panic!("expected boolean"),
    }
}

#[test]
fn symmetric_three_not_abelian() {
    let mut table = GroupTable::new();
    let g = table.permutation_group(3, &[transposition_01(3), cycle_012()]).unwrap();
    let r = athena_engine::execute_group_with_table(
        GroupRequest::IsAbelian { group: g },
        &table,
    );
    match r {
        athena_engine::GroupResult::Exact { value } => {
            assert!(matches!(value, athena_engine::GroupDomainValue::Boolean(false)));
        }
        _ => panic!("expected boolean"),
    }
}
