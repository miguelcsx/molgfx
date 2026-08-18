//! Persistent GPU storage for one cartoon representation slot.

use super::buffers::{count, upload_grow, write_draw_args};
use super::structure::GpuStructure;
use super::uniforms::ClipUniforms;
use crate::error::RenderError;
use pdviewx_core::{
    AtomSelection, ColorScheme, PlacedStructure, Representation, RepresentationKind,
};
use pdviewx_geometry::{InterpolatedCoordinates, RibbonMesh, RibbonParams, SplineProfile};
use pdviewx_gpu::{BindGroupDesc, BindGroupEntry, BufferDesc, BufferUsage, Device, Queue};

#[derive(Debug)]
pub(super) struct RibbonSlot<D: Device> {
    vertices: Option<D::Buffer>,
    indices: Option<D::Buffer>,
    args: Option<D::Buffer>,
    group: Option<D::BindGroup>,
    clipping: Option<D::Buffer>,
    vertex_capacity: u64,
    index_capacity: u64,
    index_count: u32,
}

impl<D: Device> RibbonSlot<D> {
    pub(super) fn new() -> Self {
        Self {
            vertices: None,
            indices: None,
            args: None,
            group: None,
            clipping: None,
            vertex_capacity: 0,
            index_capacity: 0,
            index_count: 0,
        }
    }

    pub(super) fn sync(&mut self, input: &mut RibbonSync<'_, D>) -> Result<(), RenderError> {
        prepare_geometry(input);
        self.index_count = count(input.mesh.indices.len());
        upload_grow(
            input.device,
            input.queue,
            "cartoon vertices",
            &input.mesh.vertices,
            &mut self.vertices,
            &mut self.vertex_capacity,
        )?;
        upload_grow(
            input.device,
            input.queue,
            "cartoon indices",
            &input.mesh.indices,
            &mut self.indices,
            &mut self.index_capacity,
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
        self.bind(input.device, input.layout, input.structure);
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

fn prepare_geometry<D: Device>(input: &mut RibbonSync<'_, D>) {
    let params = spline_params(input.representation);
    if input.representation.kind == RepresentationKind::PaperChain {
        input.mesh.vertices.clear();
        input.mesh.indices.clear();
    } else if input.representation.kind == RepresentationKind::Twister {
        input
            .mesh
            .generate_glycan(&input.placed.structure, input.selection, params);
    } else {
        match input.placed.trajectory() {
            Some(segment) => input.mesh.generate_structure_interpolated(
                &input.placed.structure,
                input.selection,
                input.placed.secondary_structure.values(),
                8.0,
                InterpolatedCoordinates {
                    start: segment.start().positions(),
                    end: segment.end().positions(),
                    alpha: segment.interpolation(),
                },
                params,
            ),
            None => input.mesh.generate_structure(
                &input.placed.structure,
                input.selection,
                input.placed.secondary_structure.values(),
                8.0,
                params,
            ),
        }
    }
    append_nucleotide_geometry(input);
    pdviewx_geometry::recolor_ribbon_with_appearance(
        &mut input.mesh.vertices,
        &input.placed.atoms,
        &input.placed.hierarchy,
        input.placed.secondary_structure.values(),
        pdviewx_geometry::PropertyColumns {
            color: input.color_property,
            appearance: input.appearance_property,
        },
        pdviewx_geometry::RibbonColoring {
            color: input.representation.color,
            appearance: input.representation.appearance,
            opacity: input.representation.material.opacity_unorm8(),
        },
    );
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
}

fn append_nucleotide_geometry<D: Device>(input: &mut RibbonSync<'_, D>) {
    if input.representation.kind == RepresentationKind::Cartoon {
        pdviewx_geometry::append_base_slabs(
            &input.placed.structure,
            input.selection,
            BASE_SLAB_THICKNESS,
            &mut input.mesh.vertices,
            &mut input.mesh.indices,
        );
    } else if input.representation.kind == RepresentationKind::PaperChain {
        pdviewx_geometry::append_base_polygons(
            &input.placed.structure,
            input.selection,
            BASE_SLAB_THICKNESS,
            0.08,
            &mut input.mesh.vertices,
            &mut input.mesh.indices,
        );
    }
}

/// Full depth of a nucleotide base slab, Ångström.
const BASE_SLAB_THICKNESS: f32 = 0.5;

fn spline_params(representation: &Representation) -> RibbonParams {
    let opacity = representation.material.opacity_unorm8();
    let color = match representation.color {
        ColorScheme::Uniform(color) => pdviewx_math::Rgba8::new(color.r, color.g, color.b, opacity),
        _ => pdviewx_math::Rgba8::new(110, 165, 235, opacity),
    };
    match representation.kind {
        RepresentationKind::Trace | RepresentationKind::Tube => {
            let diameter = representation.params.tube_radius.abs() * 2.0;
            RibbonParams {
                width: diameter,
                thickness: diameter,
                profile: SplineProfile::Tube,
                radius_mapping: representation.params.tube_radius_mapping,
                color,
                ..RibbonParams::default()
            }
        }
        RepresentationKind::Twister => RibbonParams {
            width: representation.params.ribbon_width,
            profile: SplineProfile::Tube,
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
    pub(super) layout: &'a D::BindGroupLayout,
    pub(super) structure: &'a GpuStructure<D>,
    pub(super) placed: &'a PlacedStructure,
    pub(super) representation: &'a Representation,
    pub(super) selection: &'a AtomSelection,
    pub(super) mesh: &'a mut RibbonMesh,
    pub(super) color_property: Option<&'a pdviewx_core::AtomProperty>,
    pub(super) appearance_property: Option<&'a pdviewx_core::AtomProperty>,
}
