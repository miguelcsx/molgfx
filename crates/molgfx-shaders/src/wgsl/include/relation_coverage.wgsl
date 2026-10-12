// Shared conservative proxy coverage for rendering and visibility culling.
fn relation_ndc_extent(width_pixels: f32, arrow_size: f32) -> vec2f {
    return (width_pixels * 0.5 + arrow_size * 0.75 + 1.0) * 2.0 * frame.viewport.zw;
}
