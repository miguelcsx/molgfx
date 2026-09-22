//! Persistent GPU storage for one cartoon representation slot.

use super::asset_arena::AssetArena;
use super::buffers::{count, write_draw_args};
use super::grow_buffer::GrowBuffer;
use super::structure::GpuStructure;
use super::uniforms::ClipUniforms;
use super::visual::VisualCullEntries;
use crate::error::RenderError;
use molgfx_core::{
    AtomSelection, ColorScheme, PlacedStructure, Representation, RepresentationKind,
};
use molgfx_geometry::{RibbonMesh, RibbonParams, SplineProfile};
use molgfx_gpu::{BindGroupDesc, BindGroupEntry, BufferDesc, BufferUsage, Device, Queue};

#[derive(Debug)]
pub(super) struct RibbonSlot<D: Device> {
    vertices: GrowBuffer<D>,
    indices: GrowBuffer<D>,
    deformations: GrowBuffer<D>,
    radius_sources: GrowBuffer<D>,
    args: Option<D::Buffer>,
    group: Option<D::BindGroup>,
    clipping: Option<D::Buffer>,
    index_count: u32,
}

impl<D: Device> RibbonSlot<D> {
    pub(super) fn new() -> Self {
        Self {
            vertices: GrowBuffer::new(),
            indices: GrowBuffer::new(),
            deformations: GrowBuffer::new(),
            radius_sources: GrowBuffer::new(),
            args: None,
            group: None,
            clipping: None,
            index_count: 0,
        }
    }

    pub(super) fn sync(&mut self, input: &mut RibbonSync<'_, D>) -> Result<(), RenderError> {
        prepare_geometry(input)?;
        self.index_count = count(input.mesh.indices.len());
        self.vertices.upload(
            input.device,
            input.queue,
            "cartoon vertices",
            &input.mesh.vertices,
        )?;
        self.indices.upload(
            input.device,
            input.queue,
            "cartoon indices",
            &input.mesh.indices,
        )?;
        self.deformations.upload(
            input.device,
            input.queue,
            "cartoon GPU deformation recipes",
            input.mesh.deformation_bytes(),
        )?;
        self.radius_sources.upload(
            input.device,
            input.queue,
            "cartoon guide radius sources",
            input.mesh.radius_source_values(),
        )?;
        write_draw_args(
            input.device,
            input.queue,
            "cartoon draw arguments",
            self.index_count,
            u32::from(self.index_count > 0),
            &mut self.args,
        )?;
        self.sync_clipping(input.device, input.queue, input.representation)?;
        Ok(())
    }

    pub(super) fn sync_clipping(
        &mut self,
        device: &D,
        queue: &D::Queue,
        representation: &Representation,
    ) -> Result<(), RenderError> {
        if self.clipping.is_none() {
            self.clipping = Some(device.create_buffer(&BufferDesc {
                label: "cartoon clipping uniforms",
                size: std::mem::size_of::<ClipUniforms>() as u64,
                usage: BufferUsage::UNIFORM.union(BufferUsage::COPY_DST),
            })?);
        }
        if let Some(clipping) = &self.clipping {
            queue.write_buffer(
                clipping,
                0,
                bytemuck::bytes_of(&ClipUniforms::new(representation)),
            );
        }
        Ok(())
    }

    pub(super) fn bind(
        &mut self,
        device: &D,
        layout: &D::BindGroupLayout,
        structure: &GpuStructure<D>,
        asset_arena: &AssetArena<D>,
        visual: Option<VisualCullEntries<'_, D>>,
    ) {
        let (
            Some(vertices),
            Some(indices),
            Some(model),
            Some(clipping),
            Some(deformations),
            Some(radius_sources),
            Some(visual),
        ) = (
            self.vertices.get(),
            self.indices.get(),
            &structure.model,
            &self.clipping,
            self.deformations.get(),
            self.radius_sources.get(),
            visual,
        )
        else {
            return;
        };
        self.group = Some(device.create_bind_group(&BindGroupDesc {
            label: "group2: cartoon representation",
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
                structure.coords_entry(asset_arena, 4),
                structure.previous_coords_entry(asset_arena, 5),
                BindGroupEntry::Buffer {
                    binding: 6,
                    buffer: deformations,
                },
                structure.base_coords_entry(asset_arena, 7),
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
                BindGroupEntry::Buffer {
                    binding: 13,
                    buffer: radius_sources,
                },
            ],
        }));
    }

    pub(super) fn clear(&mut self) {
        self.index_count = 0;
    }

    pub(super) fn draw(&self) -> Option<(&D::BindGroup, &D::Buffer)> {
        (self.index_count > 0).then_some((self.group.as_ref()?, self.args.as_ref()?))
    }
}

fn prepare_geometry<D: Device>(input: &mut RibbonSync<'_, D>) -> Result<(), RenderError> {
    let params = spline_params(input.representation);
    let structure = input
        .placed
        .source
        .molframe()
        .ok_or(RenderError::SourceCapabilityMissing {
            capability: "ribbon topology",
        })?;
    if input.representation.kind == RepresentationKind::PaperChain {
        input.mesh.clear();
    } else if input.representation.kind == RepresentationKind::Twister {
        input
            .mesh
            .generate_glycan(structure, input.selection, params);
    } else {
        input.mesh.generate_structure(
            structure,
            input.selection,
            input.placed.secondary_structure.values(),
            8.0,
            params,
        )?;
    }
    append_nucleotide_geometry(input)?;
    if !matches!(
        input.representation.kind,
        RepresentationKind::Twister | RepresentationKind::PaperChain
    ) {
        molgfx_geometry::recolor_ribbon_with_appearance(
            &mut input.mesh.vertices,
            &input.placed.atoms,
            &input.placed.hierarchy,
            input.placed.secondary_structure.values(),
            molgfx_geometry::PropertyColumns {
                color: input.color_property,
                appearance: input.appearance_property,
            },
            molgfx_geometry::RibbonColoring {
                color: input.representation.color,
                appearance: input.representation.appearance,
                opacity: input.representation.material.opacity_unorm8(),
            },
        );
    }
    let vertex_count = input.mesh.vertices.len();
    let index_count = input.mesh.indices.len();
    super::mesh_caps::append_caps(
        &mut input.mesh.vertices,
        &mut input.mesh.indices,
        0..vertex_count,
        0..index_count,
        input.placed.model_to_world,
        input.representation.clipping,
    );
    input.mesh.pad_static_deformations();
    Ok(())
}

fn append_nucleotide_geometry<D: Device>(input: &mut RibbonSync<'_, D>) -> Result<(), RenderError> {
    let structure = input
        .placed
        .source
        .molframe()
        .ok_or(RenderError::SourceCapabilityMissing {
            capability: "nucleotide topology",
        })?;
    if input.representation.kind == RepresentationKind::Cartoon {
        molgfx_geometry::append_base_slabs(
            structure,
            input.selection,
            BASE_SLAB_THICKNESS,
            &mut input.mesh.vertices,
            &mut input.mesh.indices,
        )?;
    } else if input.representation.kind == RepresentationKind::PaperChain {
        molgfx_geometry::append_paper_chain(
            structure,
            input.selection,
            PAPER_CHAIN_HEIGHT,
            input.representation.material.opacity_unorm8(),
            &mut input.mesh.vertices,
            &mut input.mesh.indices,
        )?;
    }
    Ok(())
}

/// Full depth of a nucleotide base slab, Ångström.
const BASE_SLAB_THICKNESS: f32 = 0.5;
/// Half-depth of a ring plate, Ångström.
///
/// A sugar ring is roughly 2.9 A across, so the former 1.0 A half-height drew a
/// 2 A tall double pyramid: a gem, not a sheet. The representation is named for
/// paper because the ring is meant to read as one, with the pucker as the bend
/// in it, so the plate is thick enough only to give its rim a lit edge.
const PAPER_CHAIN_HEIGHT: f32 = 0.09;

/// Twister ribbon width as a multiple of the configured ribbon width.
///
/// A pyranose ring is about 2.9 A across and consecutive ring centres sit near
/// 5.5 A apart, so the face has to be on that order to be a face at all. At the
/// former 0.3 A the ribbon drew as a thread: no visible plane, and therefore no
/// readable twist, which is the entire content of the representation.
const TWISTER_WIDTH_SCALE: f32 = 1.25;
/// Twister ribbon thickness as a multiple of the configured ribbon width.
///
/// The VMD reference default is 0.05 A for a 1.2 A base ribbon width.
const TWISTER_THICKNESS_SCALE: f32 = 0.2;
/// Sampling ceiling for the twisting profile.
const TWISTER_MAX_STEPS: u8 = 32;

fn spline_params(representation: &Representation) -> RibbonParams {
    let opacity = representation.material.opacity_unorm8();
    let color = match representation.color {
        ColorScheme::Uniform(color) => molgfx_math::Rgba8::new(color.r, color.g, color.b, opacity),
        _ => molgfx_math::Rgba8::new(110, 165, 235, opacity),
    };
    match representation.kind {
        RepresentationKind::Trace | RepresentationKind::Tube => {
            let diameter = representation.params.tube_radius.abs() * 2.0;
            RibbonParams {
                width: diameter,
                thickness: diameter,
                profile: SplineProfile::Tube,
                color,
                ..RibbonParams::default()
            }
        }
        // A glycan connector is flat on purpose: its face carries the relative
        // orientation of the rings it connects.
        RepresentationKind::Twister => RibbonParams {
            width: representation.params.ribbon_width * TWISTER_WIDTH_SCALE,
            thickness: representation.params.ribbon_width * TWISTER_THICKNESS_SCALE,
            profile: SplineProfile::Twister,
            // Headroom for the twist demand. Two sign-aligned ring planes can
            // sit most of a half turn apart once projected across the chord, and
            // the sampler only spends this where a pair actually does.
            max_steps: TWISTER_MAX_STEPS,
            color,
            ..RibbonParams::default()
        },
        RepresentationKind::Rocket => RibbonParams {
            width: representation.params.ribbon_width,
            profile: SplineProfile::Rocket,
            color,
            ..RibbonParams::default()
        },
        _ => RibbonParams {
            width: representation.params.ribbon_width,
            color,
            ..RibbonParams::default()
        },
    }
}

pub(super) struct RibbonSync<'a, D: Device> {
    pub(super) device: &'a D,
    pub(super) queue: &'a D::Queue,
    pub(super) placed: &'a PlacedStructure,
    pub(super) representation: &'a Representation,
    pub(super) selection: &'a AtomSelection,
    pub(super) mesh: &'a mut RibbonMesh,
    pub(super) color_property: Option<&'a molgfx_core::AtomProperty>,
    pub(super) appearance_property: Option<&'a molgfx_core::AtomProperty>,
}
