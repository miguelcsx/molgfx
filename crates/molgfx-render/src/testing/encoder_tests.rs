use super::*;
use molgfx_gpu::{
    CommandEncoder as _, Device as _, PassTimestampAbsence, TimestampCaptureError, TimestampWrites,
};

#[test]
fn unsupported_and_invalid_capture_requests_have_typed_failures() {
    assert!(matches!(
        PassTimestampCapture::new(&MockDevice::default(), 2),
        Err(TimestampCaptureError::Unsupported)
    ));
    for requested in [0, molgfx_gpu::MAX_TIMESTAMP_CAPTURE_PASSES + 1] {
        assert!(
            matches!(PassTimestampCapture::new(&MockDevice::with_timestamp_queries(), requested), Err(TimestampCaptureError::InvalidCapacity { requested: actual }) if actual == requested)
        );
    }
}

#[test]
fn mock_capture_observes_each_actual_pass_and_preserves_descriptor_precedence() {
    let device = MockDevice::with_timestamp_queries();
    let mut encoder = device.create_command_encoder();
    let _ = encoder.begin_compute_pass(&ComputePassDesc {
        label: "disabled",
        timestamps: None,
    });
    assert!(encoder.set_timestamp_capture(None).is_none());
    let capture = PassTimestampCapture::new(&device, 4).expect("capture allocates");
    assert!(encoder.set_timestamp_capture(Some(capture)).is_none());
    encoder.set_timestamp_sample(Some(7));
    let _ = encoder.begin_render_pass(&RenderPassDesc {
        label: "same label",
        colors: &[],
        depth: None,
        timestamps: None,
    });
    let _ = encoder.begin_compute_pass(&ComputePassDesc {
        label: "same label",
        timestamps: None,
    });
    encoder.set_timestamp_sample(Some(8));
    let explicit = MockQuerySet;
    let _ = encoder.begin_compute_pass(&ComputePassDesc {
        label: "caller timed",
        timestamps: Some(TimestampWrites {
            queries: &explicit,
            beginning: None,
            end: Some(0),
        }),
    });
    let _ = encoder.begin_compute_pass(&ComputePassDesc {
        label: "same label",
        timestamps: None,
    });
    let capture = encoder.set_timestamp_capture(None).expect("capture drains");
    assert_eq!(
        capture
            .records()
            .iter()
            .map(|r| (r.label, r.kind, r.sample, r.queries))
            .collect::<Vec<_>>(),
        vec![
            (
                "same label",
                TimestampPassKind::Render,
                Some(7),
                Ok(molgfx_gpu::TimestampQueryPair {
                    beginning: 0,
                    end: 1
                })
            ),
            (
                "same label",
                TimestampPassKind::Compute,
                Some(7),
                Ok(molgfx_gpu::TimestampQueryPair {
                    beginning: 2,
                    end: 3
                })
            ),
            (
                "caller timed",
                TimestampPassKind::Compute,
                Some(8),
                Err(PassTimestampAbsence::DescriptorTimestampWrites)
            ),
            (
                "same label",
                TimestampPassKind::Compute,
                Some(8),
                Ok(molgfx_gpu::TimestampQueryPair {
                    beginning: 4,
                    end: 5
                })
            ),
        ]
    );
    assert_eq!(capture.query_range(), 0..6);
}
