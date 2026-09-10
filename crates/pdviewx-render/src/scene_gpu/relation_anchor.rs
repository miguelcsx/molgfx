//! Cold-path lowering of generic spatial anchors into resolver streams.

use crate::error::RenderError;
use pdviewx_core::{Relation, RowDomain, Scene, SpatialAnchor};
use pdviewx_math::Vec3;

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub(super) enum AnchorKernel {
    World,
    Point,
    Atom,
    Rigid,
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub(super) struct StreamKey {
    pub(super) start: AnchorKernel,
    pub(super) end: AnchorKernel,
    pub(super) start_domain: Option<RowDomain>,
    pub(super) end_domain: Option<RowDomain>,
}

pub(super) fn stream_key(relation: Relation) -> StreamKey {
    StreamKey {
        start: anchor_kernel(relation.start),
        end: anchor_kernel(relation.end),
        start_domain: relation.start.source_domain(),
        end_domain: relation.end.source_domain(),
    }
}

fn anchor_kernel(anchor: SpatialAnchor) -> AnchorKernel {
    match anchor {
        SpatialAnchor::World(_) => AnchorKernel::World,
        SpatialAnchor::Entity(entity) => match entity.domain() {
            RowDomain::Atoms(_) => AnchorKernel::Atom,
            RowDomain::Points(_) => AnchorKernel::Point,
            RowDomain::Instances(_) | RowDomain::TemplateParts(_) => AnchorKernel::Rigid,
            RowDomain::Relations(_) => AnchorKernel::World,
        },
        SpatialAnchor::TemplatePart(_) => AnchorKernel::Rigid,
    }
}

pub(super) fn anchor_payload(
    scene: &Scene,
    anchor: SpatialAnchor,
) -> Result<[f32; 4], RenderError> {
    match anchor {
        SpatialAnchor::World(position) => Ok([position.x, position.y, position.z, 0.0]),
        SpatialAnchor::Entity(entity) => Ok([0.0, 0.0, 0.0, f32::from_bits(entity.row())]),
        SpatialAnchor::TemplatePart(reference) => template_part_payload(scene, reference),
    }
}

fn template_part_payload(
    scene: &Scene,
    reference: pdviewx_core::TemplatePartRef,
) -> Result<[f32; 4], RenderError> {
    let domain = RowDomain::Instances(reference.batch());
    let batch = scene
        .instance_batch(reference.batch())
        .ok_or(RenderError::RelationSourceMissing { domain })?;
    let spheres = batch.template().spheres();
    let part = reference.part_row() as usize;
    let center = if let Some(sphere) = spheres.get(part) {
        Vec3::from_array(sphere.center)
    } else {
        let capsule = batch
            .template()
            .capsules()
            .get(part.saturating_sub(spheres.len()))
            .ok_or(RenderError::RelationSourceMissing { domain })?;
        (capsule.start() + capsule.end()) * 0.5
    };
    Ok([
        center.x,
        center.y,
        center.z,
        f32::from_bits(reference.instance_row()),
    ])
}

pub(super) fn pipeline_index(key: StreamKey) -> u8 {
    kernel_index(key.start) * 4 + kernel_index(key.end)
}

const fn kernel_index(kernel: AnchorKernel) -> u8 {
    match kernel {
        AnchorKernel::World => 0,
        AnchorKernel::Point => 1,
        AnchorKernel::Atom => 2,
        AnchorKernel::Rigid => 3,
    }
}
