use super::*;

#[test]
fn host_requirements_demand_the_storage_buffer_floor_and_float_storage() {
    let required = host_requirements();
    assert_eq!(
        required.limits.max_storage_buffers_per_shader_stage,
        REQUIRED_STORAGE_BUFFERS_PER_STAGE
    );
    assert!(
        required
            .storage_texture_formats
            .contains(&wgpu::TextureFormat::R32Float)
    );
    assert!(required.features.is_empty());
}
