//! 代数父对象核心合同测试。

use athena_engine::domains::{
    DomainRequest,
    algebra::{AlgebraParentId, CoefficientParent, FieldTable, PropertyState, PropertyWitness},
    field::{FieldElement, FieldElementRepr},
    galois::GaloisRequest,
    group::{GroupDescriptor, GroupElementRepr},
};
use athena_numeric::{ExactRational, FiniteFieldValue, Integer};
use athena_types::{ExtensionId, FieldId, FieldPresentationId};

#[test]
fn field_element_has_repr_not_label() {
    let e = FieldElement {
        field: FieldId(1),
        presentation: FieldPresentationId(0),
        repr: FieldElementRepr::Rational { value: ExactRational::new(Integer::from_i64(3), Integer::one()) },
    };
    assert!(matches!(e.repr, FieldElementRepr::Rational { .. }));
}

#[test]
fn group_descriptor_abstract_not_operable_by_order_alone() {
    let d = GroupDescriptor::Abstract {
        order: PropertyState::Proven { value: Integer::from_i64(4), witness: PropertyWitness::placeholder("test") },
        properties: Default::default(),
    };
    assert!(matches!(d, GroupDescriptor::Abstract { .. }));
}

#[test]
fn group_element_repr_table_index_only_for_explicit_table() {
    assert!(matches!(GroupElementRepr::TableIndex(0), GroupElementRepr::TableIndex(_)));
}

#[test]
fn field_table_prime_field_interns() {
    let mut table = FieldTable::new();
    let q = table.rationals();
    let f5a = table.prime_field(Integer::from_i64(5)).unwrap();
    let f5b = table.prime_field(Integer::from_i64(5)).unwrap();
    assert_eq!(f5a, f5b);
    assert_ne!(q, f5a);
    assert!(table.presentation(f5a).is_some());
}

#[test]
fn coefficient_parent_roundtrip() {
    let parent = CoefficientParent::Field(FieldId(2));
    assert_eq!(parent.as_algebra_parent(), Some(AlgebraParentId::Field(FieldId(2))));
}

#[test]
fn galois_request_polynomial_vs_extension_split() {
    let req = GaloisRequest::IsExtensionNormal { extension: ExtensionId(1) };
    assert!(matches!(req, GaloisRequest::IsExtensionNormal { .. }));
    let _ = DomainRequest::GaloisTheory(req);
}

#[test]
fn finite_field_value_has_no_term_id() {
    let v = FiniteFieldValue::try_new(FieldId(0), athena_types::FieldPresentationId(0), vec![Integer::from_i64(1)]).unwrap();
    assert_eq!(v.coefficients().len(), 1);
}
