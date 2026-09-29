//! Domain requests: movie export sampling.

use super::domains::MovieExportRequest;

#[test]
fn movie_sampling_is_inclusive_and_deterministic() {
    let request = MovieExportRequest {
        start_frame: 2,
        end_frame: 4,
        frame_rate: 2.0,
        width: 16,
        height: 16,
        output_format: "mp4".into(),
        output_uri: None,
    };
    assert_eq!(
        request
            .sample_times()
            .unwrap_or_else(|error| panic!("{error}")),
        vec![1.0, 1.5, 2.0]
    );
}
