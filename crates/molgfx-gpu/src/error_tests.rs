use super::GpuError;

#[test]
fn runtime_diagnostics_have_a_distinct_code_and_preserve_backend_detail() {
    let error = GpuError::Runtime {
        detail: "buffer binding requires 48 bytes".to_owned(),
    };
    assert_eq!(error.code(), "MOLGFX-E0061");
    assert!(
        error
            .to_string()
            .contains("buffer binding requires 48 bytes")
    );
    assert_ne!(error.code(), GpuError::DeviceLost.code());
}
