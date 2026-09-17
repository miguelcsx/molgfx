//! Persistent resources for one implicit solvent-excluded representation.

use super::asset_arena::AssetArena;
use super::buffers::{buffer_entry, upload_grow};
use super::probe_offsets::probe_offsets;
use super::structure::GpuStructure;
use super::surface_components::{SurfaceComponentSync, SurfaceComponents};
use super::uniforms::RepresentationUniforms;
use crate::error::RenderError;
use crate::passes::{SurfaceComponentPass, SurfaceFieldPass};
use molgfx_core::{
    Representation, RepresentationKind, ScalarVolume, SurfaceComponentPolicy, SurfaceKind,
};
use molgfx_gpu::{
    BindGroupDesc, BindGroupEntry, Device, TextureDesc, TextureDimension, TextureFormat,
    TextureUsage, TextureViewDesc,
};
use molgfx_math::Aabb;

#[cfg(test)]
#[path = "surface_slot_tests.rs"]
mod tests;

pub(super) struct SurfaceSync<'a, D: Device> {
    pub(super) device: &'a D,
    pub(super) queue: &'a D::Queue,
    pub(super) output_layout: &'a D::BindGroupLayout,
    pub(super) input_layout: &'a D::BindGroupLayout,
    pub(super) erosion_layout: &'a D::BindGroupLayout,
    pub(super) normal_layout: &'a D::BindGroupLayout,
    pub(super) component_layout: &'a D::BindGroupLayout,
    pub(super) structure: &'a GpuStructure<D>,
    pub(super) asset_arena: &'a AssetArena<D>,
    pub(super) representation: &'a Representation,
    pub(super) atoms: &'a D::Buffer,
    pub(super) compaction: &'a D::Buffer,
    pub(super) uniforms: &'a D::Buffer,
    pub(super) atom_count: u32,
    pub(super) selection_bounds: Aabb,
    pub(super) force_generate: bool,
    pub(super) quality: bool,
    pub(super) overlay_volume: Option<&'a ScalarVolume>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct SurfaceFieldState {
    kind: u32,
    probe: u32,
    grid_min: [u32; 3],
    grid_cell: u32,
    grid_size: [u32; 4],
    components: SurfaceComponentPolicy,
}

impl SurfaceFieldState {
    fn new(uniforms: &RepresentationUniforms, representation: &Representation) -> Self {
        Self {
            kind: uniforms.options[0],
            probe: uniforms.surface[0].to_bits(),
            grid_min: [
                uniforms.grid_min[0].to_bits(),
                uniforms.grid_min[1].to_bits(),
                uniforms.grid_min[2].to_bits(),
            ],
            grid_cell: uniforms.grid_cell[0].to_bits(),
            grid_size: uniforms.grid_size,
            components: representation.params.surface_components,
        }
    }
}

#[derive(Debug)]
pub(super) struct FieldTexture<D: Device> {
    _texture: D::Texture,
    pub(super) view: D::TextureView,
}

impl<D: Device> FieldTexture<D> {
    pub(super) fn create(
        device: &D,
        dimensions: [u32; 3],
        label: &'static str,
        format: TextureFormat,
    ) -> Result<Self, RenderError> {
        let descriptor = TextureDesc {
            label,
            width: dimensions[0],
            height: dimensions[1],
            depth: dimensions[2],
            dimension: TextureDimension::D3,
            format,
            usage: TextureUsage::STORAGE_BINDING.union(TextureUsage::TEXTURE_BINDING),
        };
        let texture = device.create_texture(&descriptor)?;
        let view = device.create_texture_view(&texture, &TextureViewDesc {});
        Ok(Self {
            _texture: texture,
            view,
        })
    }
}

#[derive(Debug)]
pub(super) struct SurfaceSlot<D: Device> {
    inflated: Option<FieldTexture<D>>,
    field: Option<FieldTexture<D>>,
    normals: Option<FieldTexture<D>>,
    output: Option<D::BindGroup>,
    erosion: Option<D::BindGroup>,
    normal_output: Option<D::BindGroup>,
    erosion_offsets: Option<D::Buffer>,
    erosion_offsets_capacity: u64,
    input: Option<D::BindGroup>,
    dimensions: [u32; 3],
    pending: bool,
    enabled: bool,
    /// Which boundary this slot generates. The field algorithm follows from
    /// the kind, so it is stored once rather than as derived flags that could
    /// disagree with each other.
    kind: SurfaceKind,
    field_state: Option<SurfaceFieldState>,
    components: SurfaceComponents<D>,
}

impl<D: Device> SurfaceSlot<D> {
    pub(super) const fn new() -> Self {
        Self {
            inflated: None,
            field: None,
            normals: None,
            output: None,
            erosion: None,
            normal_output: None,
            erosion_offsets: None,
            erosion_offsets_capacity: 0,
            input: None,
            dimensions: [0; 3],
            pending: false,
            enabled: false,
            kind: SurfaceKind::SolventExcluded,
            field_state: None,
            components: SurfaceComponents::new(),
        }
    }

    /// Whether this boundary needs the reentrant erosion pass.
    ///
    /// Only the solvent-excluded surface has reentrant probe-contact patches;
    /// eroding any other field would report a boundary the caller did not ask
    /// for.
    const fn erodes(&self) -> bool {
        matches!(self.kind, SurfaceKind::SolventExcluded)
    }

    pub(super) fn sync(&mut self, sync: &SurfaceSync<'_, D>) -> Result<(), RenderError> {
        let value = RepresentationUniforms::for_quality(
            sync.representation,
            sync.selection_bounds,
            sync.overlay_volume,
            sync.quality,
        );
        let previous_enabled = self.enabled;
        let field_state = SurfaceFieldState::new(&value, sync.representation);
        self.enabled = sync.representation.kind == RepresentationKind::Surface
            && (sync.representation.params.surface_kind != SurfaceKind::VanDerWaals
                || sync.representation.params.surface_components.is_enabled())
            && sync.atom_count > 0;
        self.kind = sync.representation.params.surface_kind;
        if !self.enabled {
            self.pending = false;
            self.release_field_resources();
            self.field_state = Some(field_state);
            return Ok(());
        }
        if previous_enabled && self.field_state == Some(field_state) && !sync.force_generate {
            return Ok(());
        }
        self.field_state = Some(field_state);
        self.ensure_field(
            sync.device,
            [value.grid_size[0], value.grid_size[1], value.grid_size[2]],
        )?;
        let Some(inflated) = &self.inflated else {
            return Ok(());
        };
        self.output = Some(sync.device.create_bind_group(&BindGroupDesc {
            label: "surface field output",
            layout: sync.output_layout,
            entries: &[BindGroupEntry::Texture {
                binding: 0,
                view: &inflated.view,
            }],
        }));
        self.bind_erosion(sync, value.surface[0])?;
        self.bind_components(sync, &value)?;
        self.bind_normals(sync);
        self.input = Some(sync.device.create_bind_group(&BindGroupDesc {
            label: "surface field input",
            layout: sync.input_layout,
            entries: &[
                buffer_entry(0, sync.atoms),
                sync.structure.coords_entry(sync.asset_arena, 1),
                sync.structure.bvh_nodes_entry(sync.asset_arena, 6),
                sync.structure.bvh_indices_entry(sync.asset_arena, 7),
                buffer_entry(8, sync.compaction),
                buffer_entry(9, sync.uniforms),
            ],
        }));
        self.pending = true;
        Ok(())
    }

    fn bind_components(
        &mut self,
        sync: &SurfaceSync<'_, D>,
        uniforms: &RepresentationUniforms,
    ) -> Result<(), RenderError> {
        let source = if self.erodes() {
            &self.field
        } else {
            &self.inflated
        };
        let Some(source) = source else {
            return Ok(());
        };
        self.components.sync(&SurfaceComponentSync {
            device: sync.device,
            queue: sync.queue,
            layout: sync.component_layout,
            source: &source.view,
            dimensions: self.dimensions,
            cell: uniforms.grid_cell[0],
            isolevel: uniforms.surface[1],
            gaussian: self.kind == SurfaceKind::Gaussian,
            policy: sync.representation.params.surface_components,
        })
    }

    /// Rebuilds the erosion bind group, regenerating the rolling-probe offset
    /// table for the current probe radius.
    ///
    /// The offsets are the probe sample directions scaled by `probe`, so they
    /// must be refreshed whenever the probe changes; this runs on exactly those
    /// field regenerations.
    fn bind_erosion(&mut self, sync: &SurfaceSync<'_, D>, probe: f32) -> Result<(), RenderError> {
        let (Some(inflated), Some(field)) = (&self.inflated, &self.field) else {
            return Ok(());
        };
        let offsets = probe_offsets(probe);
        upload_grow(
            sync.device,
            sync.queue,
            "surface erosion probe offsets",
            &offsets,
            &mut self.erosion_offsets,
            &mut self.erosion_offsets_capacity,
        )?;
        let Some(erosion_offsets) = &self.erosion_offsets else {
            return Ok(());
        };
        self.erosion = Some(sync.device.create_bind_group(&BindGroupDesc {
            label: "surface field erosion",
            layout: sync.erosion_layout,
            entries: &[
                BindGroupEntry::Texture {
                    binding: 0,
                    view: &inflated.view,
                },
                BindGroupEntry::Texture {
                    binding: 1,
                    view: &field.view,
                },
                buffer_entry(2, erosion_offsets),
            ],
        }));
        Ok(())
    }

    fn bind_normals(&mut self, sync: &SurfaceSync<'_, D>) {
        let source = if let Some(filtered) = self.components.field() {
            Some(filtered)
        } else if self.erodes() {
            self.field.as_ref().map(|field| &field.view)
        } else {
            self.inflated.as_ref().map(|field| &field.view)
        };
        let (Some(source), Some(normals)) = (source, &self.normals) else {
            return;
        };
        self.normal_output = Some(sync.device.create_bind_group(&BindGroupDesc {
            label: "surface field normals",
            layout: sync.normal_layout,
            entries: &[
                BindGroupEntry::Texture {
                    binding: 0,
                    view: source,
                },
                BindGroupEntry::Texture {
                    binding: 1,
                    view: &normals.view,
                },
            ],
        }));
    }

    fn ensure_field(&mut self, device: &D, dimensions: [u32; 3]) -> Result<(), RenderError> {
        let resized = self.dimensions != dimensions;
        if self.inflated.is_none() || resized {
            let inflated = FieldTexture::create(
                device,
                dimensions,
                "probe-inflated surface field",
                TextureFormat::R32Float,
            )?;
            let field = if self.erodes() {
                Some(FieldTexture::create(
                    device,
                    dimensions,
                    "solvent-excluded surface field",
                    TextureFormat::R32Float,
                )?)
            } else {
                None
            };
            let normals = FieldTexture::create(
                device,
                dimensions,
                "continuous surface normals",
                TextureFormat::Rgba8Snorm,
            )?;
            self.output = None;
            self.release_erosion_resources();
            self.inflated = Some(inflated);
            self.field = field;
            self.normals = Some(normals);
            self.dimensions = dimensions;
        } else if self.erodes() && self.field.is_none() {
            self.field = Some(FieldTexture::create(
                device,
                dimensions,
                "solvent-excluded surface field",
                TextureFormat::R32Float,
            )?);
        } else if !self.erodes() {
            self.release_erosion_resources();
        }
        Ok(())
    }

    fn release_erosion_resources(&mut self) {
        self.erosion = None;
        self.erosion_offsets = None;
        self.erosion_offsets_capacity = 0;
        self.field = None;
    }

    fn release_field_resources(&mut self) {
        self.output = None;
        self.input = None;
        self.normal_output = None;
        self.release_erosion_resources();
        self.inflated = None;
        self.normals = None;
        self.components.release();
        self.dimensions = [0; 3];
    }

    pub(super) fn field_binding<'a>(&'a self, fallback: &'a D::TextureView) -> &'a D::TextureView {
        if self.enabled {
            if let Some(filtered) = self.components.field() {
                return filtered;
            }
            if self.erodes() {
                match &self.field {
                    Some(field) => &field.view,
                    None => fallback,
                }
            } else {
                match &self.inflated {
                    Some(field) => &field.view,
                    None => fallback,
                }
            }
        } else {
            fallback
        }
    }

    pub(super) fn normal_binding<'a>(&'a self, fallback: &'a D::TextureView) -> &'a D::TextureView {
        if self.enabled
            && let Some(normals) = &self.normals
        {
            return &normals.view;
        }
        fallback
    }

    pub(super) fn record(
        &mut self,
        encoder: &mut D::CommandEncoder,
        pass: &SurfaceFieldPass<D>,
        components: &SurfaceComponentPass<D>,
    ) {
        if !self.pending {
            return;
        }
        if let (Some(output), Some(input)) = (&self.output, &self.input) {
            pass.record_generate(
                encoder,
                output,
                input,
                self.dimensions,
                self.kind == SurfaceKind::Gaussian,
            );
            if self.erodes()
                && let Some(erosion) = &self.erosion
            {
                pass.record_erode(encoder, erosion, input, self.dimensions);
            }
            if let Some(group) = self.components.group() {
                components.record(encoder, group, self.dimensions);
            }
            if let Some(normals) = &self.normal_output {
                pass.record_normals(encoder, normals, input, self.dimensions);
            }
            self.pending = false;
        }
    }
}
