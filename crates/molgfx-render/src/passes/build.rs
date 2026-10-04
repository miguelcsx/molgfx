//! How each lazily built pass is constructed from the device and the scene's
//! bind group layouts.

use super::{
    BondPass, CartoonPass, InteractionPass, LabelPass, OitPass, PointPass, PrimitivePass,
    SpherePass, SurfacePass,
};
use crate::error::RenderError;
use crate::graph::PassEnv;
use molgfx_gpu::Device;

pub(crate) fn sphere<D: Device>(env: &PassEnv<'_, D>) -> Result<SpherePass<D>, RenderError> {
    SpherePass::new(
        env.device,
        env.target_format,
        &env.scene.group0_layout,
        &env.scene.group2_layout,
        env.scene.paged_chunk_layout(),
    )
}

pub(crate) fn point<D: Device>(env: &PassEnv<'_, D>) -> Result<PointPass<D>, RenderError> {
    PointPass::new(
        env.device,
        &env.scene.group0_layout,
        &env.scene.group2_layout,
        env.scene.paged_chunk_layout(),
        &env.scene.generic_point_render_layout,
    )
}

pub(crate) fn primitive<D: Device>(env: &PassEnv<'_, D>) -> Result<PrimitivePass<D>, RenderError> {
    PrimitivePass::new(
        env.device,
        &env.scene.group0_layout,
        &env.scene.primitive_layout,
        &env.scene.ligand_pose_layout,
        &env.scene.generic_instance_render_layout,
    )
}

pub(crate) fn surface<D: Device>(env: &PassEnv<'_, D>) -> Result<SurfacePass<D>, RenderError> {
    SurfacePass::new(
        env.device,
        &env.scene.group0_layout,
        &env.scene.group2_layout,
    )
}

pub(crate) fn bond<D: Device>(env: &PassEnv<'_, D>) -> Result<BondPass<D>, RenderError> {
    BondPass::new(
        env.device,
        env.target_format,
        &env.scene.group0_layout,
        &env.scene.group2_layout,
        env.scene.paged_bond_layout(),
    )
}

pub(crate) fn cartoon<D: Device>(env: &PassEnv<'_, D>) -> Result<CartoonPass<D>, RenderError> {
    CartoonPass::new(
        env.device,
        &env.scene.group0_layout,
        &env.scene.ribbon_layout,
    )
}

pub(crate) fn oit<D: Device>(
    env: &PassEnv<'_, D>,
    layout: &D::BindGroupLayout,
) -> Result<OitPass<D>, RenderError> {
    OitPass::new(
        env.device,
        layout,
        (
            &env.scene.group0_layout,
            &env.scene.group2_layout,
            &env.scene.ribbon_layout,
        ),
        (&env.scene.primitive_layout, &env.scene.ligand_pose_layout),
        (
            &env.scene.generic_point_render_layout,
            &env.scene.generic_instance_render_layout,
        ),
        (
            &env.scene.volume_layout,
            &env.scene.segmentation_layout,
            &env.scene.field_boundary_layout,
        ),
    )
}

pub(crate) fn interaction<D: Device>(
    env: &PassEnv<'_, D>,
) -> Result<InteractionPass<D>, RenderError> {
    InteractionPass::new(
        env.device,
        &env.scene.group0_layout,
        &env.scene.interaction_layout,
    )
}

pub(crate) fn label<D: Device>(env: &PassEnv<'_, D>) -> Result<LabelPass<D>, RenderError> {
    LabelPass::new(
        env.device,
        &env.scene.group0_layout,
        &env.scene.label_declutter_layout,
        &env.scene.label_render_layout,
    )
}
