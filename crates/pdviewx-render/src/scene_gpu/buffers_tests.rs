use super::*;

#[test]
fn growth_uses_the_device_ceiling_instead_of_crossing_it() {
    let needed = (256 << 20) + 1;
    let limit = 300 << 20;

    assert!(matches!(
        grow_capacity(needed, limit, "test buffer"),
        Ok(value) if value == limit
    ));
}

#[test]
fn growth_rejects_records_that_cannot_fit_one_storage_binding() {
    let limit = 128 << 20;

    assert!(matches!(
        grow_capacity(limit + 1, limit, "test buffer"),
        Err(RenderError::Gpu(pdviewx_gpu::GpuError::LimitExceeded {
            resource: "test buffer",
            limit: value
        })) if value == limit
    ));
}
