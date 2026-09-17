use super::{AttributeColumn, AttributeDescriptor, AttributeKind, AttributeValues};
use crate::{PointBatchHandle, RowDomain, handle::RawHandle};
use std::sync::Arc;

fn domain() -> RowDomain {
    RowDomain::Points(PointBatchHandle(RawHandle::new_for_test(1, 0)))
}

#[test]
fn descriptors_retain_generic_units_and_provenance_in_the_fingerprint() {
    let values = AttributeValues::Scalar(Arc::from([1.0]));
    let plain = AttributeColumn::new(domain(), "score", values.clone()).unwrap();
    let described = AttributeColumn::with_descriptor(
        domain(),
        AttributeDescriptor::new("score")
            .with_quantity("energy", "kcal/mol")
            .with_provenance("caller:method/v1"),
        values,
    )
    .unwrap();
    assert_eq!(described.descriptor().quantity(), Some("energy"));
    assert_eq!(described.descriptor().unit(), Some("kcal/mol"));
    assert_eq!(
        described.descriptor().provenance(),
        Some("caller:method/v1")
    );
    assert_ne!(plain.fingerprint(), described.fingerprint());
}

#[test]
fn every_attribute_keeps_its_native_row_width() {
    let scalar = AttributeColumn::new(
        domain(),
        "score",
        AttributeValues::Scalar(Arc::from([1.0, 2.0])),
    )
    .unwrap();
    let vector = AttributeColumn::new(
        domain(),
        "gradient",
        AttributeValues::Vector(Arc::from([[1.0, 2.0, 3.0]])),
    )
    .unwrap();

    assert_eq!(scalar.kind(), AttributeKind::Scalar);
    assert_eq!(scalar.stride(), 4);
    assert_eq!(scalar.values().as_bytes().len(), 8);
    assert_eq!(vector.stride(), 12);
    assert_eq!(vector.values().as_bytes().len(), 12);
}

#[test]
fn attributes_retain_shared_values_and_validate_missing_vectors() {
    let values: Arc<[f32]> = Arc::from([1.0, f32::NAN]);
    let column = AttributeColumn::new(
        domain(),
        "optional",
        AttributeValues::Scalar(Arc::clone(&values)),
    )
    .unwrap();
    let AttributeValues::Scalar(stored) = column.values() else {
        panic!("expected scalar values");
    };
    assert!(Arc::ptr_eq(stored, &values));
    assert!(
        AttributeColumn::new(
            domain(),
            "bad vector",
            AttributeValues::Vector(Arc::from([[f32::NAN, 0.0, f32::NAN]])),
        )
        .is_err()
    );
}
