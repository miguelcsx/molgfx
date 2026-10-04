//! Source-revision-owned categorical boundary buffers, shared by styles.

use super::buffers::{upload_grow, write_draw_args};
use crate::RenderError;
use molgfx_core::SegmentedVolume;
use molgfx_gpu::{
    BindGroupDesc, BindGroupEntry, BindGroupLayoutDesc, BindGroupLayoutEntry, BindingType, Device,
    ShaderStages,
};

pub(crate) enum SegmentationGeometry<'a, D: Device> {
    Proxy,
    Boundary(&'a D::BindGroup, &'a D::Buffer),
}

#[derive(Debug)]
pub(super) struct GpuBoundary<D: Device> {
    vertices: Option<D::Buffer>,
    vertex_capacity: u64,
    indices: Option<D::Buffer>,
    index_capacity: u64,
    arguments: Option<D::Buffer>,
    group: Option<D::BindGroup>,
    source_revision: Option<u64>,
    vertex_count: u32,
}

impl<D: Device> Default for GpuBoundary<D> {
    fn default() -> Self {
        Self {
            vertices: None,
            vertex_capacity: 0,
            indices: None,
            index_capacity: 0,
            arguments: None,
            group: None,
            source_revision: None,
            vertex_count: 0,
        }
    }
}

pub(super) fn layout<D: Device>(device: &D) -> D::BindGroupLayout {
    device.create_bind_group_layout(&BindGroupLayoutDesc {
        label: "group3: indexed categorical boundary",
        entries: &[0, 1].map(|binding| BindGroupLayoutEntry {
            binding,
            visibility: ShaderStages::VERTEX,
            ty: BindingType::Storage { read_only: true },
        }),
    })
}

impl<D: Device> GpuBoundary<D> {
    pub(super) fn sync(
        &mut self,
        device: &D,
        queue: &D::Queue,
        layout: &D::BindGroupLayout,
        source: &SegmentedVolume,
        revision: u64,
    ) -> Result<bool, RenderError> {
        if self.source_revision == Some(revision) {
            return Ok(false);
        }
        let mesh = molgfx_geometry::extract_label_surfaces(source)?;
        let vertex_count = mesh
            .triangles
            .len()
            .checked_mul(3)
            .and_then(|count| u32::try_from(count).ok())
            .ok_or(molgfx_geometry::FieldError::IndexOverflow)?;
        upload_grow(
            device,
            queue,
            "categorical boundary vertices",
            &mesh.vertices,
            &mut self.vertices,
            &mut self.vertex_capacity,
        )?;
        upload_grow(
            device,
            queue,
            "categorical boundary indices",
            &mesh.triangles,
            &mut self.indices,
            &mut self.index_capacity,
        )?;
        write_draw_args(
            device,
            queue,
            "categorical boundary draw arguments",
            vertex_count,
            1,
            &mut self.arguments,
        )?;
        let (Some(vertices), Some(indices)) = (&self.vertices, &self.indices) else {
            return Err(RenderError::Residency {
                reason: "categorical boundary buffers are unavailable",
            });
        };
        self.group = Some(device.create_bind_group(&BindGroupDesc {
            label: "indexed categorical boundary",
            layout,
            entries: &[
                BindGroupEntry::Buffer {
                    binding: 0,
                    buffer: vertices,
                },
                BindGroupEntry::Buffer {
                    binding: 1,
                    buffer: indices,
                },
            ],
        }));
        self.vertex_count = vertex_count;
        self.source_revision = Some(revision);
        Ok(true)
    }

    pub(super) fn draw(&self) -> Option<(&D::BindGroup, &D::Buffer)> {
        (self.vertex_count > 0).then_some((self.group.as_ref()?, self.arguments.as_ref()?))
    }
}
