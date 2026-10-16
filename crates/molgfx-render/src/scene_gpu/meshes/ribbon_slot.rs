//! Persistent GPU storage for one cartoon representation slot.

use crate::scene_gpu::asset_arena::AssetArena;
use crate::scene_gpu::buffers::buffer_entry;
use crate::scene_gpu::buffers::count;
use crate::scene_gpu::grow_buffer::GrowBuffer;
use crate::scene_gpu::indexed_draw::{IndexedDraw, write_arguments};
use crate::scene_gpu::structure::GpuStructure;
use crate::scene_gpu::uniforms::ClipUniforms;
use crate::scene_gpu::visual::VisualCullEntries;
mod helix;
mod molecular_surface;
use crate::error::RenderError;
use molgfx_core::{
    AtomSelection, ColorScheme, PlacedStructure, Representation, RepresentationKind,
};
use molgfx_geometry::{RibbonMesh, RibbonParams, SplineProfile};
use molgfx_gpu::{BindGroupDesc, BufferDesc, BufferUsage, Device, Queue};

#[derive(Debug)]
pub(in crate::scene_gpu) struct RibbonSlot<D: Device> {
    vertices: GrowBuffer<D>,
    indices: GrowBuffer<D>,
    deformations: GrowBuffer<D>,
    radius_sources: GrowBuffer<D>,
    fit_jobs: GrowBuffer<D>,
    fit_group: Option<D::BindGroup>,
    fit_count: u32,
    args: Option<D::Buffer>,
    group: Option<D::BindGroup>,
    clipping: Option<D::Buffer>,
    distinct_previous: bool,
    index_count: u32,
}

impl<D: Device> RibbonSlot<D> {
    pub(in crate::scene_gpu) fn new() -> Self {
        Self {
            vertices: GrowBuffer::new(),
            indices: GrowBuffer::index(),
            deformations: GrowBuffer::new(),
            radius_sources: GrowBuffer::new(),
            fit_jobs: GrowBuffer::new(),
            fit_group: None,
            fit_count: 0,
            args: None,
            group: None,
            clipping: None,
            distinct_previous: false,
            index_count: 0,
        }
    }

    pub(in crate::scene_gpu) fn sync(
        &mut self,
        input: &mut RibbonSync<'_, D>,
    ) -> Result<(), RenderError> {
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
        self.fit_count = match input.mesh.helix_fit_jobs().first() {
            Some(count) => *count,
            None => 0,
        };
        self.fit_jobs.upload(
            input.device,
            input.queue,
            "helix fit jobs",
            input.mesh.helix_fit_jobs(),
        )?;
        write_arguments(
            input.device,
            input.queue,
            "cartoon draw arguments",
            self.index_count,
            &mut self.args,
        )?;
        self.sync_clipping(
            input.device,
            input.queue,
            input.representation,
            input.distinct_previous,
        )?;
        if input.mesh.vertices.capacity() > RETAINED_SCRATCH_VERTICES {
            *input.mesh = RibbonMesh::default();
        }
        Ok(())
    }

    pub(in crate::scene_gpu) fn sync_clipping(
        &mut self,
        device: &D,
        queue: &D::Queue,
        representation: &Representation,
        distinct_previous: bool,
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
        self.distinct_previous = false;
        self.set_distinct_previous(queue, distinct_previous);
        Ok(())
    }

    /// Tells the vertex stage whether the previous coordinates differ from the
    /// current ones; when they do not, it deforms each vertex once.
    pub(in crate::scene_gpu) fn set_distinct_previous(&mut self, queue: &D::Queue, distinct: bool) {
        if self.distinct_previous == distinct {
            return;
        }
        if let Some(clipping) = &self.clipping {
            queue.write_buffer(
                clipping,
                ClipUniforms::DISTINCT_PREVIOUS_OFFSET,
                bytemuck::bytes_of(&if distinct { 2.0_f32 } else { 1.0_f32 }),
            );
            self.distinct_previous = distinct;
        }
    }

    pub(in crate::scene_gpu) fn bind(
        &mut self,
        device: &D,
        layout: &D::BindGroupLayout,
        structure: &GpuStructure<D>,
        asset_arena: &AssetArena<D>,
        visual: Option<VisualCullEntries<'_, D>>,
    ) {
        let (
            Some(vertices),
            Some(model),
            Some(clipping),
            Some(deformations),
            Some(radius_sources),
            Some(visual),
        ) = (
            self.vertices.get(),
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
                buffer_entry(0, vertices),
                buffer_entry(2, model),
                buffer_entry(3, clipping),
                structure.coords_entry(asset_arena, 4),
                structure.previous_coords_entry(asset_arena, 5),
                buffer_entry(6, deformations),
                structure.base_coords_entry(asset_arena, 7),
                buffer_entry(8, visual.results),
                buffer_entry(9, visual.instructions),
                buffer_entry(10, visual.parameters),
                buffer_entry(11, visual.properties),
                buffer_entry(12, visual.config),
                buffer_entry(13, radius_sources),
            ],
        }));
    }

    pub(in crate::scene_gpu) fn draw(&self) -> Option<IndexedDraw<'_, D>> {
        (self.index_count > 0).then_some(IndexedDraw {
            group: self.group.as_ref()?,
            arguments: self.args.as_ref()?,
            indices: self.indices.get()?,
        })
    }
}

fn prepare_geometry<D: Device>(input: &mut RibbonSync<'_, D>) -> Result<(), RenderError> {
    let params = spline_params(input.representation, input.ribbon_steps);
    if input.representation.kind == RepresentationKind::MolecularSurface {
        molecular_surface::prepare(input, params.color)?;
    } else if input.representation.kind == RepresentationKind::Polyhedra {
        input.mesh.generate_polyhedra(
            input.placed,
            input.selection,
            input.representation.params.distance_cutoff,
            params.color,
        )?;
    } else {
        let structure =
            input
                .placed
                .source
                .molframe()
                .ok_or(RenderError::SourceCapabilityMissing {
                    capability: "ribbon topology",
                })?;
        if matches!(
            input.representation.kind,
            RepresentationKind::PaperChain
                | RepresentationKind::Bases
                | RepresentationKind::BasePairs
        ) {
            input.mesh.clear();
        } else if input.representation.kind == RepresentationKind::Ribbons {
            input.mesh.generate_legacy_ribbons(
                structure,
                input.selection,
                molgfx_geometry::CARTOON_GAP_CUTOFF,
                params,
                input.representation.params.bond_radius,
            )?;
        } else if input.representation.kind == RepresentationKind::Twister {
            input
                .mesh
                .generate_glycan(structure, input.selection, params)?;
        } else {
            input.mesh.generate_structure(
                structure,
                input.selection,
                input.placed.secondary_structure.values(),
                molgfx_geometry::CARTOON_GAP_CUTOFF,
                params,
            )?;
        }
        if input.representation.params.gaps == molgfx_core::GapStyle::Dashed {
            input.mesh.append_polymer_gaps(
                structure,
                input.selection,
                input.placed.trajectory(),
                params,
            )?;
        }
        append_nucleotide_geometry(input)?;
    }
    if !matches!(
        input.representation.kind,
        RepresentationKind::Twister | RepresentationKind::PaperChain
    ) {
        molgfx_geometry::recolor_ribbon(
            &mut input.mesh.vertices,
            &input.placed.atoms,
            input.color,
            molgfx_geometry::RibbonColoring {
                color: input.representation.color,
                overlay: input.representation.color_overlay,
                appearance: input.representation.appearance,
                opacity: u8::MAX,
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
    input.mesh.prepare_helix_fits()?;
    Ok(())
}

fn append_nucleotide_geometry<D: Device>(input: &mut RibbonSync<'_, D>) -> Result<(), RenderError> {
    let start = input.mesh.vertices.len();
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
        input.mesh.generate_ring_bipyramids(
            structure,
            input.selection,
            input.representation.params.ribbon_width,
            input.representation.params.max_ring_size,
            u8::MAX,
        )?;
    } else if matches!(
        input.representation.kind,
        RepresentationKind::Bases | RepresentationKind::BasePairs
    ) {
        let paired = if input.representation.kind == RepresentationKind::BasePairs {
            Some(crate::scene_gpu::nucleotide_pairs::selected_pairs(
                structure,
                input.selection,
            )?)
        } else {
            None
        };
        let selection = match &paired {
            Some(selection) => selection,
            None => input.selection,
        };
        molgfx_geometry::append_base_polygons(
            structure,
            selection,
            input.representation.params.radius_scale,
            if paired.is_some() {
                input.representation.params.bond_radius
            } else {
                BASE_OUTLINE_RADIUS
            },
            &mut input.mesh.vertices,
            &mut input.mesh.indices,
        )?;
    }
    if input.representation.kind != RepresentationKind::PaperChain {
        input.mesh.anchor_nucleotide_vertices(structure, start);
    }
    Ok(())
}

/// Full depth of a nucleotide base slab, Ångström.
const BASE_SLAB_THICKNESS: f32 = 0.5;
const BASE_OUTLINE_RADIUS: f32 = 0.08;
/// Ring connector width relative to the authored ribbon width.
const TWISTER_WIDTH_SCALE: f32 = 1.25;
/// Ring connector thickness relative to the authored ribbon width.
const TWISTER_THICKNESS_SCALE: f32 = 0.2;
/// Sampling ceiling for the twisting profile.
const TWISTER_MAX_STEPS: u8 = 32;

/// Host scratch above this many vertices is released after upload; a larger
/// ribbon is rebuilt on the rare edit that changes it.
const RETAINED_SCRATCH_VERTICES: usize = 32_768;

pub(in crate::scene_gpu) fn spline_params(
    representation: &Representation,
    steps: u8,
) -> RibbonParams {
    let color = match representation.color {
        ColorScheme::Uniform(color) => molgfx_math::Rgba8::new(color.r, color.g, color.b, u8::MAX),
        _ => molgfx_math::Rgba8::new(110, 165, 235, u8::MAX),
    };
    match representation.kind {
        RepresentationKind::Trace | RepresentationKind::Tube => {
            let diameter = representation.params.tube_radius.abs() * 2.0;
            RibbonParams {
                width: diameter,
                thickness: diameter,
                profile: if representation.kind == RepresentationKind::Trace {
                    SplineProfile::Trace
                } else {
                    SplineProfile::Tube
                },
                max_steps: steps,
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
        RepresentationKind::NewRibbons => RibbonParams {
            width: representation.params.ribbon_width,
            thickness: representation.params.ribbon_width
                / representation.params.cartoon_aspect_ratio,
            profile: SplineProfile::NewRibbons,
            helix_profile: representation.params.cartoon_helix_profile,
            arrow_factor: 0.0,
            max_steps: steps,
            color,
            ..RibbonParams::default()
        },
        RepresentationKind::Rocket => RibbonParams {
            helix_radius: representation.params.cartoon_helix_radius,
            width: representation.params.ribbon_width,
            thickness: representation.params.ribbon_width
                / representation.params.cartoon_aspect_ratio,
            profile: SplineProfile::Rocket,
            aspect_ratio: representation.params.cartoon_aspect_ratio,
            arrow_factor: representation.params.cartoon_arrow_factor,
            direction_wedges: representation.params.cartoon_direction_wedges,
            helix_profile: representation.params.cartoon_helix_profile,
            nucleic_profile: representation.params.cartoon_nucleic_profile,
            max_steps: steps,
            color,
            ..RibbonParams::default()
        },
        _ => RibbonParams {
            width: representation.params.ribbon_width,
            aspect_ratio: representation.params.cartoon_aspect_ratio,
            arrow_factor: representation.params.cartoon_arrow_factor,
            direction_wedges: representation.params.cartoon_direction_wedges,
            helix_profile: representation.params.cartoon_helix_profile,
            nucleic_profile: representation.params.cartoon_nucleic_profile,
            max_steps: steps,
            color,
            ..RibbonParams::default()
        },
    }
}

pub(in crate::scene_gpu) struct RibbonSync<'a, D: Device> {
    /// Maximum samples per trace interval the quality tier allows.
    pub(in crate::scene_gpu) ribbon_steps: u8,
    pub(in crate::scene_gpu) device: &'a D,
    pub(in crate::scene_gpu) queue: &'a D::Queue,
    pub(in crate::scene_gpu) placed: &'a PlacedStructure,
    pub(in crate::scene_gpu) representation: &'a Representation,
    pub(in crate::scene_gpu) selection: &'a AtomSelection,
    pub(in crate::scene_gpu) mesh: &'a mut RibbonMesh,
    /// Whether the structure carries previous-sample coordinates.
    pub(in crate::scene_gpu) distinct_previous: bool,
    /// The columns the colour scheme, its overlay and its appearance read.
    pub(in crate::scene_gpu) color: molgfx_geometry::ColorContext<'a>,
}

#[cfg(test)]
#[path = "ribbon_slot/params_tests.rs"]
mod params_tests;
