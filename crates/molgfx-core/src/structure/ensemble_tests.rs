use super::*;
use crate::handle::RawHandle;

fn member(index: u32) -> StructureHandle {
    StructureHandle(RawHandle::new_for_test(index, 0))
}

#[test]
fn weights_normalize_and_ties_keep_caller_order() {
    let ensemble = Ensemble::new(
        Arc::from([member(0), member(1), member(2)]),
        &[2.0, 3.0, 3.0],
        "caller:clusters/v1",
    )
    .unwrap_or_else(|error| panic!("{error}"));
    assert!((ensemble.weights().iter().sum::<f32>() - 1.0).abs() < 1.0e-6);
    assert_eq!(ensemble.dominant_index(), 1);
}

#[test]
fn malformed_members_weights_and_provenance_are_typed_errors() {
    let duplicate = Ensemble::new(Arc::from([member(0), member(0)]), &[0.5, 0.5], "caller");
    let negative = Ensemble::new(Arc::from([member(0)]), &[-1.0], "caller");
    let absent = Ensemble::new(Arc::from([member(0)]), &[1.0], "");
    assert!(matches!(duplicate, Err(CoreError::InvalidEnsemble { .. })));
    assert!(matches!(negative, Err(CoreError::InvalidEnsemble { .. })));
    assert!(matches!(absent, Err(CoreError::InvalidEnsemble { .. })));
}
