//! Source-owned indexed field buffers, independent of presentation styles.

use super::buffers::{upload_grow, write_draw_args};
use crate::RenderError;
use molgfx_core::{ScalarVolume, SegmentedVolume};
use molgfx_gpu::{
    BindGroupDesc, BindGroupEntry, BindGroupLayoutDesc, BindGroupLayoutEntry, BindingType, Device,
    ShaderStages,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum GeometryKind {
    Proxy,
    Boundary,
}

pub(crate) enum FieldGeometry<'a, D: Device> {
    Proxy,
    Boundary(&'a D::BindGroup, &'a D::Buffer),
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
struct BoundaryKey {
    revision: u64,
    level: Option<u32>,
}

#[derive(Clone, Copy)]
pub(super) enum BoundarySource<'a> {
    Labels(&'a SegmentedVolume),
    Scalar { grid: &'a ScalarVolume, level: f32 },
}

#[derive(Debug)]
pub(super) struct GpuFieldBoundary<D: Device> {
    vertices: Option<D::Buffer>,
    vertex_capacity: u64,
    indices: Option<D::Buffer>,
    index_capacity: u64,
    arguments: Option<D::Buffer>,
    group: Option<D::BindGroup>,
    key: Option<BoundaryKey>,
    vertex_count: u32,
}

impl<D: Device> Default for GpuFieldBoundary<D> {
    fn default() -> Self {
        Self {
            vertices: None,
            vertex_capacity: 0,
            indices: None,
            index_capacity: 0,
            arguments: None,
            group: None,
            key: None,
            vertex_count: 0,
        }
    }
}

pub(super) fn layout<D: Device>(device: &D) -> D::BindGroupLayout {
    device.create_bind_group_layout(&BindGroupLayoutDesc {
        label: "group3: indexed field boundary",
        entries: &[0, 1].map(|binding| BindGroupLayoutEntry {
            binding,
            visibility: ShaderStages::VERTEX,
            ty: BindingType::Storage { read_only: true },
        }),
    })
}

impl<D: Device> GpuFieldBoundary<D> {
    pub(super) fn sync(
        &mut self,
        device: &D,
        queue: &D::Queue,
        layout: &D::BindGroupLayout,
        source: BoundarySource<'_>,
        revision: u64,
    ) -> Result<bool, RenderError> {
        let key = BoundaryKey {
            revision,
            level: match source {
                BoundarySource::Labels(_) => None,
                BoundarySource::Scalar { level, .. } => Some(level.to_bits()),
            },
        };
        if self.key == Some(key) {
            return Ok(false);
        }
        let mesh = match source {
            BoundarySource::Labels(grid) => molgfx_geometry::extract_label_surfaces(grid)?,
            BoundarySource::Scalar { grid, level } => {
                molgfx_geometry::extract_isosurface(grid, level)?
            }
        };
        let vertex_count = mesh
            .triangles
            .len()
            .checked_mul(3)
            .and_then(|count| u32::try_from(count).ok())
            .ok_or(molgfx_geometry::FieldError::IndexOverflow)?;
        upload_grow(
            device,
            queue,
            "field boundary vertices",
            &mesh.vertices,
            &mut self.vertices,
            &mut self.vertex_capacity,
        )?;
        upload_grow(
            device,
            queue,
            "field boundary indices",
            &mesh.triangles,
            &mut self.indices,
            &mut self.index_capacity,
        )?;
        write_draw_args(
            device,
            queue,
            "field boundary draw arguments",
            vertex_count,
            1,
            &mut self.arguments,
        )?;
        let (Some(vertices), Some(indices)) = (&self.vertices, &self.indices) else {
            return Err(RenderError::Residency {
                reason: "field boundary buffers are unavailable",
            });
        };
        self.group = Some(device.create_bind_group(&BindGroupDesc {
            label: "indexed field boundary",
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
        self.key = Some(key);
        Ok(true)
    }

    pub(super) fn draw(&self) -> Option<(&D::BindGroup, &D::Buffer)> {
        (self.vertex_count > 0).then_some((self.group.as_ref()?, self.arguments.as_ref()?))
    }
}

impl<D: Device> FieldGeometry<'_, D> {
    pub(crate) const fn kind(&self) -> GeometryKind {
        match self {
            Self::Proxy => GeometryKind::Proxy,
            Self::Boundary(..) => GeometryKind::Boundary,
        }
    }
}
