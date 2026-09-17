//! Persistent GPU-only temporal occupancy accumulation.

use super::asset_arena::AssetArena;
use super::buffers::buffer_entry;
use super::structure::GpuStructure;
use super::volume_slot::OccupancySync;
use crate::error::RenderError;
use crate::passes::OccupancyPass;
use molgfx_core::PlacedStructure;
use molgfx_gpu::{
    BindGroupDesc, BindGroupEntry, BufferDesc, BufferUsage, ComputePassEncoder, Device, Queue,
    TextureDesc, TextureDimension, TextureFormat, TextureUsage, TextureViewDesc,
};

const FIXED_SCALE: f32 = 4096.0;

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct OccupancyUniforms {
    model_to_voxel: [f32; 16],
    dimensions: [u32; 4],
    macro_dimensions: [u32; 4],
    parameters: [f32; 4],
    counts: [u32; 4],
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
struct SampleKey {
    coordinates: u64,
    trajectory: u64,
}

struct OccupancyShape {
    dimensions: [u32; 3],
    macro_dimensions: [u32; 3],
    voxel_count: u32,
    macro_count: u32,
    selected_count: u32,
    selected_bytes: u64,
}

struct OccupancyResources<D: Device> {
    accumulator: D::Buffer,
    selected_rows: D::Buffer,
    uniforms: D::Buffer,
    texture: D::Texture,
    view: D::TextureView,
    bounds_texture: D::Texture,
    bounds_view: D::TextureView,
}

#[derive(Debug)]
pub(super) struct GpuOccupancy<D: Device> {
    accumulator: D::Buffer,
    selected_rows: D::Buffer,
    uniforms: D::Buffer,
    _texture: D::Texture,
    view: D::TextureView,
    _bounds_texture: D::Texture,
    bounds_view: D::TextureView,
    group: Option<D::BindGroup>,
    voxel_count: u32,
    selected_count: u32,
    macro_count: u32,
    structure_binding: u64,
    sample: Option<SampleKey>,
    needs_clear: bool,
    dirty: bool,
}

impl<D: Device> GpuOccupancy<D> {
    pub(super) fn new(input: &OccupancySync<'_, D>) -> Result<Self, RenderError> {
        let device = input.device;
        let queue = input.queue;
        let layout = input.layout;
        let stream = input.stream;
        let selected_rows = input.selected_rows;
        let placed = input.placed;
        let structure = input.structure;
        let asset_arena = input.asset_arena;
        let shape = occupancy_shape(device, stream.dimensions(), selected_rows.len())?;
        let resources = create_resources(device, &shape)?;
        if !selected_rows.is_empty() {
            queue.write_buffer(
                &resources.selected_rows,
                0,
                bytemuck::cast_slice(selected_rows),
            );
        }
        queue.write_buffer(
            &resources.uniforms,
            0,
            bytemuck::bytes_of(&occupancy_uniforms(stream, &shape)),
        );
        let mut value = Self {
            accumulator: resources.accumulator,
            selected_rows: resources.selected_rows,
            uniforms: resources.uniforms,
            _texture: resources.texture,
            view: resources.view,
            _bounds_texture: resources.bounds_texture,
            bounds_view: resources.bounds_view,
            group: None,
            voxel_count: shape.voxel_count,
            selected_count: shape.selected_count,
            macro_count: shape.macro_count,
            structure_binding: structure.binding_revision,
            sample: None,
            needs_clear: true,
            dirty: true,
        };
        value.bind(device, layout, structure, asset_arena);
        value.mark_sample(placed);
        Ok(value)
    }

    pub(super) fn sync(
        &mut self,
        device: &D,
        layout: &D::BindGroupLayout,
        placed: &PlacedStructure,
        structure: &GpuStructure<D>,
        asset_arena: &AssetArena<D>,
    ) {
        if self.structure_binding != structure.binding_revision {
            self.structure_binding = structure.binding_revision;
            self.bind(device, layout, structure, asset_arena);
        }
        self.mark_sample(placed);
    }

    fn mark_sample(&mut self, placed: &PlacedStructure) {
        let sample = SampleKey {
            coordinates: placed.atoms.coords().generation(),
            trajectory: placed.trajectory_revision(),
        };
        if self.sample != Some(sample) {
            self.sample = Some(sample);
            self.dirty = true;
        }
    }

    fn bind(
        &mut self,
        device: &D,
        layout: &D::BindGroupLayout,
        structure: &GpuStructure<D>,
        asset_arena: &AssetArena<D>,
    ) {
        self.group = Some(device.create_bind_group(&BindGroupDesc {
            label: "temporal occupancy accumulation",
            layout,
            entries: &[
                buffer_entry(0, &self.accumulator),
                buffer_entry(1, &self.selected_rows),
                structure.coords_entry(asset_arena, 2),
                buffer_entry(3, &self.uniforms),
                BindGroupEntry::Texture {
                    binding: 4,
                    view: &self.view,
                },
                BindGroupEntry::Texture {
                    binding: 5,
                    view: &self.bounds_view,
                },
            ],
        }));
    }

    pub(super) fn record<P: ComputePassEncoder<D>>(
        &mut self,
        pass: &mut P,
        pipelines: &OccupancyPass<D>,
    ) {
        if !self.dirty {
            return;
        }
        let Some(group) = &self.group else {
            return;
        };
        pass.set_bind_group(0, group, &[]);
        pass.set_pipeline(if self.needs_clear {
            &pipelines.clear
        } else {
            &pipelines.decay
        });
        dispatch(pass, self.voxel_count);
        if self.selected_count > 0 {
            pass.set_pipeline(&pipelines.deposit);
            dispatch(pass, self.selected_count);
        }
        pass.set_pipeline(&pipelines.resolve);
        dispatch(pass, self.voxel_count);
        pass.set_pipeline(&pipelines.bounds);
        dispatch(pass, self.macro_count);
        self.needs_clear = false;
        self.dirty = false;
    }

    pub(super) const fn is_dirty(&self) -> bool {
        self.dirty && self.group.is_some()
    }

    pub(super) fn view(&self) -> &D::TextureView {
        &self.view
    }

    pub(super) fn bounds_view(&self) -> &D::TextureView {
        &self.bounds_view
    }
}

fn occupancy_shape<D: Device>(
    device: &D,
    dimensions: [u32; 3],
    selected_len: usize,
) -> Result<OccupancyShape, RenderError> {
    if dimensions
        .iter()
        .any(|&dimension| dimension > device.capabilities().max_texture_dim_3d)
    {
        return Err(limit_error(
            "occupancy 3-D texture",
            u64::from(device.capabilities().max_texture_dim_3d),
        ));
    }
    let brick = molgfx_core::ScalarVolume::EMPTY_SPACE_BRICK_SIZE;
    let mut macro_dimensions = [0; 3];
    for (output, dimension) in macro_dimensions.iter_mut().zip(dimensions) {
        *output = dimension
            .checked_add(brick - 1)
            .ok_or_else(|| limit_error("occupancy macro dimensions", u64::from(u32::MAX)))?
            / brick;
    }
    let selected_count = u32::try_from(selected_len)
        .map_err(|_| limit_error("occupancy selected rows", u64::from(u32::MAX)))?;
    let selected_bytes = u64::from(selected_count)
        .checked_mul(std::mem::size_of::<u32>() as u64)
        .ok_or_else(|| limit_error("occupancy selected row bytes", u64::MAX))?
        .max(std::mem::size_of::<u32>() as u64);
    Ok(OccupancyShape {
        dimensions,
        macro_dimensions,
        voxel_count: product(dimensions)?,
        macro_count: product(macro_dimensions)?,
        selected_count,
        selected_bytes,
    })
}

fn create_resources<D: Device>(
    device: &D,
    shape: &OccupancyShape,
) -> Result<OccupancyResources<D>, RenderError> {
    let accumulator = device.create_buffer(&BufferDesc {
        label: "temporal occupancy fixed-point grid",
        size: u64::from(shape.voxel_count) * std::mem::size_of::<u32>() as u64,
        usage: BufferUsage::STORAGE,
    })?;
    let selected_rows = device.create_buffer(&BufferDesc {
        label: "temporal occupancy selected rows",
        size: shape.selected_bytes,
        usage: BufferUsage::STORAGE.union(BufferUsage::COPY_DST),
    })?;
    let uniforms = device.create_buffer(&BufferDesc {
        label: "temporal occupancy uniforms",
        size: std::mem::size_of::<OccupancyUniforms>() as u64,
        usage: BufferUsage::UNIFORM.union(BufferUsage::COPY_DST),
    })?;
    let texture = create_texture(
        device,
        "temporal occupancy density",
        shape.dimensions,
        TextureFormat::R32Float,
    )?;
    let view = device.create_texture_view(&texture, &TextureViewDesc::default());
    let bounds_texture = create_texture(
        device,
        "temporal occupancy bounds",
        shape.macro_dimensions,
        TextureFormat::Rg32Float,
    )?;
    let bounds_view = device.create_texture_view(&bounds_texture, &TextureViewDesc::default());
    Ok(OccupancyResources {
        accumulator,
        selected_rows,
        uniforms,
        texture,
        view,
        bounds_texture,
        bounds_view,
    })
}

fn occupancy_uniforms(
    stream: &molgfx_core::OccupancyStream,
    shape: &OccupancyShape,
) -> OccupancyUniforms {
    OccupancyUniforms {
        model_to_voxel: stream.voxel_to_model().inverse().to_cols_array(),
        dimensions: [
            shape.dimensions[0],
            shape.dimensions[1],
            shape.dimensions[2],
            0,
        ],
        macro_dimensions: [
            shape.macro_dimensions[0],
            shape.macro_dimensions[1],
            shape.macro_dimensions[2],
            0,
        ],
        parameters: [
            stream.decay(),
            stream.deposit() * FIXED_SCALE,
            stream.maximum() * FIXED_SCALE,
            FIXED_SCALE.recip(),
        ],
        counts: [
            shape.selected_count,
            shape.voxel_count,
            shape.macro_count,
            molgfx_core::ScalarVolume::EMPTY_SPACE_BRICK_SIZE,
        ],
    }
}

fn limit_error(resource: &'static str, limit: u64) -> RenderError {
    molgfx_gpu::GpuError::LimitExceeded { resource, limit }.into()
}

fn create_texture<D: Device>(
    device: &D,
    label: &'static str,
    dimensions: [u32; 3],
    format: TextureFormat,
) -> Result<D::Texture, RenderError> {
    Ok(device.create_texture(&TextureDesc {
        label,
        width: dimensions[0],
        height: dimensions[1],
        depth: dimensions[2],
        dimension: TextureDimension::D3,
        format,
        usage: TextureUsage::TEXTURE_BINDING.union(TextureUsage::STORAGE_BINDING),
    })?)
}

fn product(dimensions: [u32; 3]) -> Result<u32, RenderError> {
    dimensions
        .into_iter()
        .try_fold(1u32, u32::checked_mul)
        .ok_or(
            molgfx_gpu::GpuError::LimitExceeded {
                resource: "occupancy voxel count",
                limit: u64::from(u32::MAX),
            }
            .into(),
        )
}

fn dispatch<D: Device, P: ComputePassEncoder<D>>(pass: &mut P, count: u32) {
    let groups = super::dispatch::workgroups_2d(u64::from(count).div_ceil(64));
    pass.dispatch(groups[0], groups[1], 1);
}
