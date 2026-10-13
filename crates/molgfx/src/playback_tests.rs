use crate::TrajectoryFrame;
use crate::playback::{FrameCache, Timeline, TimelineError};
use std::sync::Arc;

#[test]
fn timelines_validate_counts_and_order() {
    assert_eq!(Timeline::new(1, vec![0.0]), Err(TimelineError::FrameCount));
    assert_eq!(
        Timeline::new(3, vec![0.0, 1.0]),
        Err(TimelineError::FrameCount)
    );
    assert_eq!(
        Timeline::new(3, vec![0.0, 2.0, 1.0]),
        Err(TimelineError::NotAscending)
    );
    assert!(Timeline::new(3, vec![0.0, 1.0, 2.0]).is_ok());
}

#[test]
fn locate_clamps_and_picks_the_containing_segment() {
    let Ok(line) = Timeline::new(3, vec![0.0, 1.0, 2.0]) else {
        panic!("valid timeline")
    };
    assert_eq!(line.locate(0.5).segment, 0);
    assert_eq!(line.locate(1.0).segment, 1);
    assert_eq!(line.locate(9.0).segment, 1);
    assert!((line.locate(9.0).time - 2.0).abs() < f32::EPSILON);
    assert!(line.locate(-3.0).time.abs() < f32::EPSILON);
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
