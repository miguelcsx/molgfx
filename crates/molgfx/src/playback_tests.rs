use crate::TrajectoryFrame;
use crate::playback::{FrameCache, FrameTable, FrameTableError};
use crate::time::SimTime;
use std::sync::Arc;

fn table(frames: usize, seconds: &[f64]) -> Result<FrameTable, FrameTableError> {
    FrameTable::new(frames, seconds.iter().copied().map(SimTime::from).collect())
}

#[test]
fn frame_tables_validate_counts_and_order() {
    assert_eq!(table(1, &[0.0]), Err(FrameTableError::FrameCount));
    assert_eq!(table(3, &[0.0, 1.0]), Err(FrameTableError::FrameCount));
    assert_eq!(
        table(3, &[0.0, 2.0, 1.0]),
        Err(FrameTableError::NotAscending)
    );
    assert!(table(3, &[0.0, 1.0, 2.0]).is_ok());
}

#[test]
fn unrecorded_frame_times_are_rejected_as_non_finite() {
    // molframe reports an unrecorded frame as NaN; a NaN gap must not pass as ascending.
    assert_eq!(
        table(3, &[0.0, f64::NAN, 2.0]),
        Err(FrameTableError::NonFinite)
    );
    assert_eq!(
        table(3, &[0.0, 1.0, f64::INFINITY]),
        Err(FrameTableError::NonFinite)
    );
}

#[test]
fn locate_clamps_and_picks_the_containing_segment() {
    let Ok(line) = table(3, &[0.0, 1.0, 2.0]) else {
        panic!("valid frame table")
    };
    assert_eq!(line.locate(SimTime::from(0.5)).segment, 0);
    assert_eq!(line.locate(SimTime::from(1.0)).segment, 1);
    assert_eq!(line.locate(SimTime::from(9.0)).segment, 1);
    assert!((line.locate(SimTime::from(9.0)).time.seconds() - 2.0).abs() < f64::EPSILON);
    assert!(line.locate(SimTime::from(-3.0)).time.seconds().abs() < f64::EPSILON);
}

#[test]
fn cache_converts_each_recent_frame_once() {
    let mut cache = FrameCache::default();
    let mut conversions = 0;
    for index in [0, 1, 0, 1, 2] {
        let result: Result<_, ()> = cache.get_or_convert(index, || {
            conversions += 1;
            Ok(TrajectoryFrame::new(
                index as u64,
                0.0,
                Arc::from([[0.0; 3]]),
            ))
        });
        assert!(result.is_ok());
    }
    assert_eq!(conversions, 3);
}
