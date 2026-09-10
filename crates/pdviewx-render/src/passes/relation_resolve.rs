//! Load-time pipelines for homogeneous dynamic-relation anchor streams.

use crate::error::RenderError;
use pdviewx_gpu::{ComputePipelineDesc, Device, ShaderModuleDesc};

const ENTRIES: [&str; 16] = [
    "resolve_world_world",
    "resolve_world_point",
    "resolve_world_atom",
    "resolve_world_rigid",
    "resolve_point_world",
    "resolve_point_point",
    "resolve_point_atom",
    "resolve_point_rigid",
    "resolve_atom_world",
    "resolve_atom_point",
    "resolve_atom_atom",
    "resolve_atom_rigid",
    "resolve_rigid_world",
    "resolve_rigid_point",
    "resolve_rigid_atom",
    "resolve_rigid_rigid",
];

#[derive(Debug)]
pub struct RelationResolvePass<D: Device> {
    pipelines: Vec<D::Pipeline>,
}

impl<D: Device> RelationResolvePass<D> {
    pub fn new(device: &D, layout: &D::BindGroupLayout) -> Result<Self, RenderError> {
        let shader = device.create_shader_module(&ShaderModuleDesc {
            label: "dynamic relation resolver",
            wgsl: pdviewx_shaders::RELATION_RESOLVE,
        })?;
        let mut pipelines = Vec::with_capacity(ENTRIES.len());
        for entry in ENTRIES {
            pipelines.push(device.create_compute_pipeline(&ComputePipelineDesc {
                label: "dynamic relation resolver",
                layouts: &[None, Some(layout)],
                shader: &shader,
                entry,
            })?);
        }
        Ok(Self { pipelines })
    }

    pub(crate) fn pipeline(&self, index: usize) -> Option<&D::Pipeline> {
        self.pipelines.get(index)
    }
}
