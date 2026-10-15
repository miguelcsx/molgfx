//! What a timeline dispatch binds.

use molgfx_gpu::Device;

#[derive(Clone, Copy)]
pub(crate) struct AttributeTimelineDispatch<'a, D: Device> {
    pub(crate) group: &'a D::BindGroup,
    pub(crate) groups: [u32; 2],
}
