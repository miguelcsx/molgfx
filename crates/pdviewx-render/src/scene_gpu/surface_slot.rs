//! Persistent resources for one implicit solvent-excluded representation.

use super::buffers::buffer_entry;
use super::structure::GpuStructure;
use super::uniforms::RepresentationUniforms;
use crate::error::RenderError;
use crate::passes::SurfaceFieldPass;
use pdviewx_core::{DensityVolume, Representation, RepresentationKind, SurfaceKind};
use pdviewx_gpu::{
    BindGroupDesc, BindGroupEntry, Device, TextureDesc, TextureDimension, TextureFormat,
    TextureUsage, TextureViewDesc,
};

pub(super) struct SurfaceSync<'a, D: Device> {
    pub(super) device: &'a D,
    pub(super) output_layout: &'a D::BindGroupLayout,
    pub(super) input_layout: &'a D::BindGroupLayout,
    pub(super) erosion_layout: &'a D::BindGroupLayout,
    pub(super) structure: &'a GpuStructure<D>,
    pub(super) representation: &'a Representation,
    pub(super) atoms: &'a D::Buffer,
    pub(super) compaction: &'a D::Buffer,
    pub(super) uniforms: &'a D::Buffer,
    pub(super) atom_count: u32,
    pub(super) force_generate: bool,
    pub(super) overlay_volume: Option<&'a DensityVolume>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct SurfaceFieldState {
    kind: u32,
    probe: u32,
    grid_min: [u32; 3],
    grid_cell: u32,
    grid_size: [u32; 4],
}

impl SurfaceFieldState {
    fn new(uniforms: &RepresentationUniforms) -> Self {
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
        }
    }
}

#[derive(Debug)]
pub(super) struct SurfaceSlot<D: Device> {
    inflated: Option<D::Texture>,
    inflated_view: Option<D::TextureView>,
    inflated_provenance: Option<D::Texture>,
    inflated_provenance_view: Option<D::TextureView>,
    field: Option<D::Texture>,
    field_view: Option<D::TextureView>,
    field_provenance: Option<D::Texture>,
    field_provenance_view: Option<D::TextureView>,
    output: Option<D::BindGroup>,
    erosion: Option<D::BindGroup>,
    input: Option<D::BindGroup>,
    dimensions: [u32; 3],
    cells: u32,
    pending: bool,
    enabled: bool,
    /// Which boundary this slot generates. The field algorithm follows from
    /// the kind, so it is stored once rather than as derived flags that could
    /// disagree with each other.
    kind: SurfaceKind,
    field_state: Option<SurfaceFieldState>,
}

impl<D: Device> SurfaceSlot<D> {
    pub(super) const fn new() -> Self {
        Self {
            inflated: None,
            inflated_view: None,
            inflated_provenance: None,
            inflated_provenance_view: None,
            field: None,
            field_view: None,
            field_provenance: None,
            field_provenance_view: None,
            output: None,
            erosion: None,
            input: None,
            dimensions: [0; 3],
            cells: 0,
            pending: false,
            enabled: false,
            kind: SurfaceKind::SolventExcluded,
            field_state: None,
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
        let value = RepresentationUniforms::new(
            sync.representation,
            sync.structure.bvh_bounds(),
            sync.overlay_volume,
        );
        let previous_enabled = self.enabled;
        let field_state = SurfaceFieldState::new(&value);
        self.enabled = sync.representation.kind == RepresentationKind::Surface
            && sync.representation.params.surface_kind != SurfaceKind::VanDerWaals
            && sync.atom_count > 0;
        self.kind = sync.representation.params.surface_kind;
        if !self.enabled {
            self.pending = false;
            self.field_state = Some(field_state);
            return Ok(());
        }
        if previous_enabled && self.field_state == Some(field_state) && !sync.force_generate {
            return Ok(());
        }
        self.field_state = Some(field_state);
        self.cells = value.grid_size[3];
        self.ensure_field(
            sync.device,
            [value.grid_size[0], value.grid_size[1], value.grid_size[2]],
        )?;
        let (Some(inflated), Some(inflated_id), Some(field), Some(field_id)) = (
            &self.inflated_view,
            &self.inflated_provenance_view,
            &self.field_view,
            &self.field_provenance_view,
        ) else {
            return Ok(());
        };
        self.output = Some(sync.device.create_bind_group(&BindGroupDesc {
            label: "surface field output",
            layout: sync.output_layout,
            entries: &[
                BindGroupEntry::Texture {
                    binding: 0,
                    view: inflated,
                },
                BindGroupEntry::Texture {
                    binding: 1,
                    view: inflated_id,
                },
            ],
        }));
        self.erosion = Some(sync.device.create_bind_group(&BindGroupDesc {
            label: "surface field erosion",
            layout: sync.erosion_layout,
            entries: &[
                BindGroupEntry::Texture {
                    binding: 0,
                    view: inflated,
                },
                BindGroupEntry::Texture {
                    binding: 1,
                    view: field,
                },
                BindGroupEntry::Texture {
                    binding: 2,
                    view: inflated_id,
                },
                BindGroupEntry::Texture {
                    binding: 3,
                    view: field_id,
                },
            ],
        }));
        let (Some(coords), Some(nodes), Some(indices)) = (
            sync.structure.coords(),
            &sync.structure.bvh_nodes,
            &sync.structure.bvh_indices,
        ) else {
            return Ok(());
        };
        self.input = Some(sync.device.create_bind_group(&BindGroupDesc {
            label: "surface field input",
            layout: sync.input_layout,
            entries: &[
                buffer_entry(0, sync.atoms),
                buffer_entry(1, coords),
                buffer_entry(6, nodes),
                buffer_entry(7, indices),
                buffer_entry(8, sync.compaction),
                buffer_entry(9, sync.uniforms),
            ],
        }));
        self.pending = true;
        Ok(())
    }

    fn ensure_field(&mut self, device: &D, dimensions: [u32; 3]) -> Result<(), RenderError> {
        if self.field.is_none() || self.inflated.is_none() || self.dimensions != dimensions {
            self.dimensions = dimensions;
            let descriptor = |label, format| TextureDesc {
                label,
                width: dimensions[0],
                height: dimensions[1],
                depth: dimensions[2],
                dimension: TextureDimension::D3,
                format,
                usage: TextureUsage::STORAGE_BINDING.union(TextureUsage::TEXTURE_BINDING),
            };
            let inflated = device.create_texture(&descriptor(
                "probe-inflated surface field",
                TextureFormat::R32Float,
            ))?;
            let inflated_provenance = device.create_texture(&descriptor(
                "probe-inflated surface provenance",
                TextureFormat::R32Uint,
            ))?;
            let field = device.create_texture(&descriptor(
                "solvent-excluded surface field",
                TextureFormat::R32Float,
            ))?;
            let field_provenance = device.create_texture(&descriptor(
                "solvent-excluded surface provenance",
                TextureFormat::R32Uint,
            ))?;
            self.inflated_view = Some(device.create_texture_view(&inflated, &TextureViewDesc {}));
            self.inflated_provenance_view =
                Some(device.create_texture_view(&inflated_provenance, &TextureViewDesc {}));
            self.field_view = Some(device.create_texture_view(&field, &TextureViewDesc {}));
            self.field_provenance_view =
                Some(device.create_texture_view(&field_provenance, &TextureViewDesc {}));
            self.inflated = Some(inflated);
            self.inflated_provenance = Some(inflated_provenance);
            self.field = Some(field);
            self.field_provenance = Some(field_provenance);
        }
        Ok(())
    }

    pub(super) fn field_binding<'a>(&'a self, fallback: &'a D::TextureView) -> &'a D::TextureView {
        if self.enabled {
            if self.erodes() {
                match &self.field_view {
                    Some(field) => field,
                    None => fallback,
                }
            } else {
                match &self.inflated_view {
                    Some(field) => field,
                    None => fallback,
                }
            }
        } else {
            fallback
        }
    }

    pub(super) fn provenance_binding<'a>(
        &'a self,
        fallback: &'a D::TextureView,
    ) -> &'a D::TextureView {
        if self.enabled {
            if self.erodes() {
                match &self.field_provenance_view {
                    Some(view) => view,
                    None => fallback,
                }
            } else {
                match &self.inflated_provenance_view {
                    Some(view) => view,
                    None => fallback,
                }
            }
        } else {
            fallback
        }
    }

    pub(super) fn record(&mut self, encoder: &mut D::CommandEncoder, pass: &SurfaceFieldPass<D>) {
        if !self.pending {
            return;
        }
        if let (Some(output), Some(input)) = (&self.output, &self.input) {
            pass.record_generate(
                encoder,
                output,
                input,
                self.cells,
                self.kind == SurfaceKind::Gaussian,
            );
            if self.erodes()
                && let Some(erosion) = &self.erosion
            {
                pass.record_erode(encoder, erosion, input, self.cells);
            }
            self.pending = false;
        }
    }
}
