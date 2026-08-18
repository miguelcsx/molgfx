use super::*;

#[test]
fn every_buffer_usage_bit_maps_to_its_wgpu_bit() {
    let all = BufferUsage::all();
    let mapped = buffer_usage(all);
    assert!(mapped.contains(wgpu::BufferUsages::VERTEX));
    assert!(mapped.contains(wgpu::BufferUsages::STORAGE));
    assert!(mapped.contains(wgpu::BufferUsages::INDIRECT));
    assert!(mapped.contains(wgpu::BufferUsages::MAP_READ));
    assert!(mapped.contains(wgpu::BufferUsages::QUERY_RESOLVE));
    assert_eq!(
        buffer_usage(BufferUsage::empty()),
        wgpu::BufferUsages::empty()
    );
}

#[test]
fn depth_format_round_trips_and_reports_itself_as_depth() {
    assert!(TextureFormat::Depth32Float.is_depth());
    assert_eq!(
        texture_format(TextureFormat::Depth32Float),
        wgpu::TextureFormat::Depth32Float
    );
}

#[test]
fn compact_motion_format_maps_to_two_half_float_channels() {
    assert_eq!(
        texture_format(TextureFormat::Rg16Float),
        wgpu::TextureFormat::Rg16Float
    );
    assert_eq!(surface_format_back(wgpu::TextureFormat::Rg16Float), None);
}

#[test]
fn swapchain_formats_convert_back_and_offscreen_formats_do_not() {
    for format in [
        TextureFormat::Bgra8Unorm,
        TextureFormat::Bgra8UnormSrgb,
        TextureFormat::Rgba8Unorm,
        TextureFormat::Rgba8UnormSrgb,
    ] {
        assert_eq!(surface_format_back(texture_format(format)), Some(format));
    }
    assert_eq!(surface_format_back(wgpu::TextureFormat::Rgba16Float), None);
}

#[test]
fn replace_blending_is_no_blend_state_at_all() {
    assert!(blend_state(BlendMode::Replace).is_none());
    assert!(blend_state(BlendMode::Alpha).is_some());
}

#[test]
fn reversed_depth_comparison_maps_to_greater_equal() {
    assert_eq!(
        compare_function(CompareFunction::GreaterEqual),
        wgpu::CompareFunction::GreaterEqual
    );
}
