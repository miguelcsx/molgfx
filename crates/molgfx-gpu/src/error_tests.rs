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

#[test]
fn host_context_errors_list_every_unmet_requirement() {
    let error = GpuError::HostRequirements {
        missing: vec!["limit a".to_owned(), "limit b".to_owned()],
    };
    assert_eq!(error.code(), "MOLGFX-E0012");
    let text = error.to_string();
    assert!(text.contains("limit a") && text.contains("limit b"));
    let target = GpuError::HostTarget {
        problems: vec!["no render attachment".to_owned()],
    };
    assert_eq!(target.code(), "MOLGFX-E0013");
}
