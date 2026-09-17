//! Persistent GPU storage for one caller-supplied mesh.
//!
//! A mesh is not generated from a structure, so it has no representation slot
//! to live in. It does share the generated cartoon's vertex layout, which is
//! the point: it binds the same group and draws with the same indexed pipeline,
//! so caller triangles cost no extra pass, shader or pipeline.

use super::buffers::{count, upload_grow, write_draw_args};
use super::structure::GpuStructure;
use super::uniforms::ClipUniforms;
use super::visual::VisualCullEntries;
use crate::error::RenderError;
use molgfx_core::{EntityId, Mesh};
use molgfx_geometry::RibbonVertex;
use molgfx_gpu::{BindGroupDesc, BindGroupEntry, BufferDesc, BufferUsage, Device, Queue};
use molgfx_math::Mat4;

/// One resident caller mesh.
#[derive(Debug)]
pub(super) struct GpuMeshSlot<D: Device> {
    vertices: Option<D::Buffer>,
    indices: Option<D::Buffer>,
    args: Option<D::Buffer>,
    clipping: Option<D::Buffer>,
    group: Option<D::BindGroup>,
    vertex_capacity: u64,
    index_capacity: u64,
    index_count: u32,
    translucent: bool,
    scratch: Vec<RibbonVertex>,
    index_scratch: Vec<u32>,
}

#[derive(Clone, Copy)]
pub(super) struct MeshOccurrences<'a> {
    pub(super) transforms: &'a [Mat4],
    pub(super) model_to_world: Mat4,
    pub(super) entity: EntityId,
}

impl<D: Device> GpuMeshSlot<D> {
    pub(super) fn new() -> Self {
        Self {
            vertices: None,
            indices: None,
            args: None,
            clipping: None,
            group: None,
            vertex_capacity: 0,
            index_capacity: 0,
            index_count: 0,
            translucent: false,
            scratch: Vec::new(),
            index_scratch: Vec::new(),
        }
    }

    /// Uploads the caller's triangles, converting them into the shared cartoon
    /// vertex. The scratch buffer is retained, so a re-sync of an unchanged
    /// mesh size allocates nothing.
    pub(super) fn sync(
        &mut self,
        device: &D,
        queue: &D::Queue,
        mesh: &Mesh,
        bindings: (
            &D::BindGroupLayout,
            &GpuStructure<D>,
            VisualCullEntries<'_, D>,
        ),
        occurrences: MeshOccurrences<'_>,
    ) -> Result<(), RenderError> {
        let (layout, structure, visual) = bindings;
        self.translucent = mesh.material().is_translucent()
            || mesh.vertices().iter().any(|vertex| vertex.color.a < 255);
        let opacity = mesh.material().opacity_unorm8();
        self.scratch.clear();
        self.index_scratch.clear();
        for transform in
            std::iter::once(Mat4::IDENTITY).chain(occurrences.transforms.iter().copied())
        {
            let vertex_start = self.scratch.len();
            let index_start = self.index_scratch.len();
            let base = count(self.scratch.len());
            self.scratch.extend(mesh.vertices().iter().map(|vertex| {
                let mut color = vertex.color;
                color.a = multiply_unorm8(color.a, opacity);
                let normal = transform
                    .transform_vector3(vertex.normal)
                    .try_normalize()
                    .map_or(vertex.normal, |unit| unit);
                RibbonVertex {
                    position: transform.transform_point3(vertex.position).to_array(),
                    entity_id: occurrences.entity.0,
                    normal: normal.to_array(),
                    color,
                }
            }));
            self.index_scratch.extend(
                mesh.indices()
                    .iter()
                    .map(|index| index.saturating_add(base)),
            );
            let vertex_end = self.scratch.len();
            let index_end = self.index_scratch.len();
            super::mesh_caps::append_caps(
                &mut self.scratch,
                &mut self.index_scratch,
                vertex_start..vertex_end,
                index_start..index_end,
                occurrences.model_to_world,
                mesh.clipping(),
            );
        }
        self.index_count = count(self.index_scratch.len());
        upload_grow(
            device,
            queue,
            "caller mesh vertices",
            &self.scratch,
            &mut self.vertices,
            &mut self.vertex_capacity,
        )?;
        upload_grow(
            device,
            queue,
            "caller mesh indices",
            &self.index_scratch,
            &mut self.indices,
            &mut self.index_capacity,
        )?;
        write_draw_args(
            device,
            queue,
            "caller mesh draw arguments",
            self.index_count,
            u32::from(self.index_count > 0),
            &mut self.args,
        )?;
        self.sync_clipping(device, queue, mesh)?;
        self.bind(device, layout, structure, &visual);
        Ok(())
    }

    fn sync_clipping(
        &mut self,
        device: &D,
        queue: &D::Queue,
        mesh: &Mesh,
    ) -> Result<(), RenderError> {
        if self.clipping.is_none() {
            self.clipping = Some(device.create_buffer(&BufferDesc {
                label: "caller mesh clipping uniforms",
                size: std::mem::size_of::<ClipUniforms>() as u64,
                usage: BufferUsage::UNIFORM.union(BufferUsage::COPY_DST),
            })?);
        }
        if let Some(clipping) = &self.clipping {
            queue.write_buffer(
                clipping,
                0,
                bytemuck::bytes_of(&ClipUniforms::for_mesh(mesh)),
            );
        }
        Ok(())
    }

    fn bind(
        &mut self,
        device: &D,
        layout: &D::BindGroupLayout,
        structure: &GpuStructure<D>,
        visual: &VisualCullEntries<'_, D>,
    ) {
        let (Some(vertices), Some(indices), Some(model), Some(clipping)) = (
            &self.vertices,
            &self.indices,
            &structure.model,
            &self.clipping,
        ) else {
            return;
        };
        self.group = Some(device.create_bind_group(&BindGroupDesc {
            label: "group2: caller mesh",
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
                BindGroupEntry::Buffer {
                    binding: 2,
                    buffer: model,
                },
                BindGroupEntry::Buffer {
                    binding: 3,
                    buffer: clipping,
                },
                BindGroupEntry::Buffer {
                    binding: 8,
                    buffer: visual.results,
                },
                BindGroupEntry::Buffer {
                    binding: 9,
                    buffer: visual.instructions,
                },
                BindGroupEntry::Buffer {
                    binding: 10,
                    buffer: visual.parameters,
                },
                BindGroupEntry::Buffer {
                    binding: 11,
                    buffer: visual.properties,
                },
                BindGroupEntry::Buffer {
                    binding: 12,
                    buffer: visual.config,
                },
                // Static caller meshes never enable tube mapping. Reusing the
                // vertex allocation satisfies the shared layout without
                // creating a per-mesh dummy resource.
                BindGroupEntry::Buffer {
                    binding: 13,
                    buffer: vertices,
                },
            ],
        }));
    }

    /// The bind group and indirect arguments for one pass, or nothing when the
    /// mesh is empty or belongs to the other transparency stream.
    pub(super) fn draw(&self, translucent: bool) -> Option<(&D::BindGroup, &D::Buffer)> {
        if self.translucent != translucent || self.index_count == 0 {
            return None;
        }
        self.group.as_ref().zip(self.args.as_ref())
    }

    pub(super) const fn is_translucent(&self) -> bool {
        self.translucent && self.index_count > 0
    }
}

fn multiply_unorm8(left: u8, right: u8) -> u8 {
    let product = u16::from(left) * u16::from(right) + 127;
    u8::try_from(product / 255).map_or(u8::MAX, |value| value)
}
