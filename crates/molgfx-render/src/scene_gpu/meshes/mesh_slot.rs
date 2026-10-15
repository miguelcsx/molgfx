//! Persistent GPU storage for one caller-supplied mesh.
//!
//! A mesh is not generated from a structure, so it has no representation slot
//! to live in. It does share the generated cartoon's vertex layout, which is
//! the point: it binds the same group and draws with the same indexed pipeline,
//! so caller triangles cost no extra pass, shader or pipeline.

use crate::error::RenderError;
use crate::scene_gpu::buffers::{buffer_entry, count, upload_grow};
use crate::scene_gpu::grow_buffer::GrowBuffer;
use crate::scene_gpu::indexed_draw::{IndexedDraw, write_arguments};
use crate::scene_gpu::structure::GpuStructure;
use crate::scene_gpu::uniforms::ClipUniforms;
use crate::scene_gpu::visual::VisualCullEntries;
use molgfx_core::{EntityId, Mesh};
use molgfx_geometry::RibbonVertex;
use molgfx_gpu::{BindGroupDesc, BufferDesc, BufferUsage, Device, Queue};
use molgfx_math::Mat4;

/// One resident caller mesh.
#[derive(Debug)]
pub(in crate::scene_gpu) struct GpuMeshSlot<D: Device> {
    vertices: Option<D::Buffer>,
    indices: GrowBuffer<D>,
    args: Option<D::Buffer>,
    clipping: Option<D::Buffer>,
    group: Option<D::BindGroup>,
    vertex_capacity: u64,
    index_count: u32,
    translucent: bool,
    scratch: Vec<RibbonVertex>,
    index_scratch: Vec<u32>,
}

#[derive(Clone, Copy)]
pub(in crate::scene_gpu) struct MeshOccurrences<'a> {
    pub(in crate::scene_gpu) transforms: &'a [Mat4],
    pub(in crate::scene_gpu) model_to_world: Mat4,
    pub(in crate::scene_gpu) entity: EntityId,
}

impl<D: Device> GpuMeshSlot<D> {
    pub(in crate::scene_gpu) fn new() -> Self {
        Self {
            vertices: None,
            indices: GrowBuffer::index(),
            args: None,
            clipping: None,
            group: None,
            vertex_capacity: 0,
            index_count: 0,
            translucent: false,
            scratch: Vec::new(),
            index_scratch: Vec::new(),
        }
    }

    /// Uploads the caller's triangles, converting them into the shared cartoon
    /// vertex. The scratch buffer is retained, so a re-sync of an unchanged
    /// mesh size allocates nothing.
    pub(in crate::scene_gpu) fn sync(
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
                    .into_iter()
                    .fold(vertex.normal, |_, unit| unit);
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
        self.indices
            .upload(device, queue, "caller mesh indices", &self.index_scratch)?;
        write_arguments(
            device,
            queue,
            "caller mesh draw arguments",
            self.index_count,
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
        let (Some(vertices), Some(model), Some(clipping)) =
            (&self.vertices, &structure.model, &self.clipping)
        else {
            return;
        };
        self.group = Some(device.create_bind_group(&BindGroupDesc {
            label: "group2: caller mesh",
            layout,
            entries: &[
                buffer_entry(0, vertices),
                buffer_entry(2, model),
                buffer_entry(3, clipping),
                // Static meshes disable spline deformation before any storage
                // read. Reuse the vertex buffer for the unused shared bindings.
                buffer_entry(4, vertices),
                buffer_entry(5, vertices),
                buffer_entry(6, vertices),
                buffer_entry(7, vertices),
                buffer_entry(8, visual.results),
                buffer_entry(9, visual.instructions),
                buffer_entry(10, visual.parameters),
                buffer_entry(11, visual.properties),
                buffer_entry(12, visual.config),
                // Static caller meshes never enable tube mapping. Reusing the
                // vertex allocation satisfies the shared layout without
                // creating a per-mesh dummy resource.
                buffer_entry(13, vertices),
            ],
        }));
    }

    /// The bind group and indirect arguments for one pass, or nothing when the
    /// mesh is empty or belongs to the other transparency stream.
    pub(in crate::scene_gpu) fn draw(&self, translucent: bool) -> Option<IndexedDraw<'_, D>> {
        if self.translucent != translucent || self.index_count == 0 {
            return None;
        }
        Some(IndexedDraw {
            group: self.group.as_ref()?,
            arguments: self.args.as_ref()?,
            indices: self.indices.get()?,
        })
    }

    pub(in crate::scene_gpu) const fn is_translucent(&self) -> bool {
        self.translucent && self.index_count > 0
    }
}

fn multiply_unorm8(left: u8, right: u8) -> u8 {
    let product = u16::from(left) * u16::from(right) + 127;
    crate::fallback(u8::try_from(product / 255), u8::MAX)
}
