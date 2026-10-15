use super::*;
use crate::testing::{MockBindGroupLayout, MockDevice};

#[test]
fn quality_and_portable_realtime_pipelines_share_the_pass_contract() {
    let device = MockDevice::default();
    AmbientOcclusionPass::new(
        &device,
        &MockBindGroupLayout::default(),
        &MockBindGroupLayout::default(),
    )
    .unwrap_or_else(|error| panic!("AO pipelines initialize: {error}"));
}
