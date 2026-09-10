//! GPU visibility compaction and indirect argument generation.

use crate::error::RenderError;
use crate::graph::PassContext;
use pdviewx_gpu::{
    CommandEncoder as _, ComputePassDesc, ComputePassEncoder, ComputePipelineDesc, Device,
    ShaderModuleDesc,
};

#[derive(Debug)]
pub struct CullPass<D: Device> {
    reset: D::Pipeline,
    reset_tiles: D::Pipeline,
    bin_atoms: D::Pipeline,
    compact_tiles: D::Pipeline,
    atoms: D::Pipeline,
    bonds: D::Pipeline,
    direct_bonds: D::Pipeline,
    visual_cull: D::Pipeline,
    visual_shading: D::Pipeline,
    visual_shading_all: D::Pipeline,
    paged_reset: D::Pipeline,
    paged_trajectory: D::Pipeline,
    paged_cull: D::Pipeline,
    paged_bond_reset: D::Pipeline,
    paged_bond_cull: D::Pipeline,
    generic_point_reset: D::Pipeline,
    generic_point_cull: D::Pipeline,
    generic_point_compact: D::Pipeline,
    generic_point_visual_cull: D::Pipeline,
    generic_point_visual_shading: D::Pipeline,
    generic_instance_reset: D::Pipeline,
    generic_instance_cull: D::Pipeline,
    generic_instance_visual_cull: D::Pipeline,
    generic_instance_visual_shading: D::Pipeline,
    relation_reset: D::Pipeline,
    relation_cull: D::Pipeline,
    point_timeline: D::Pipeline,
    instance_timeline: D::Pipeline,
    attribute_timeline: D::Pipeline,
}

#[derive(Clone, Copy)]
pub(crate) struct CullLayouts<'a, D: Device> {
    pub(crate) atoms: &'a D::BindGroupLayout,
    pub(crate) bonds: &'a D::BindGroupLayout,
    pub(crate) visuals: &'a D::BindGroupLayout,
    pub(crate) frame: &'a D::BindGroupLayout,
    pub(crate) paged: &'a D::BindGroupLayout,
    pub(crate) paged_bonds: &'a D::BindGroupLayout,
    pub(crate) points: &'a D::BindGroupLayout,
    pub(crate) instances: &'a D::BindGroupLayout,
    pub(crate) relations: &'a D::BindGroupLayout,
    pub(crate) instance_timeline: &'a D::BindGroupLayout,
    pub(crate) attribute_timeline: &'a D::BindGroupLayout,
}

impl<D: Device> CullPass<D> {
    pub(crate) fn new(device: &D, layouts: &CullLayouts<'_, D>) -> Result<Self, RenderError> {
        let base = BasePipelines::new(device, layouts.atoms, layouts.bonds, layouts.visuals)?;
        let paged = PagedPipelines::new(device, layouts.frame, layouts.paged, layouts.paged_bonds)?;
        let points = PointPipelines::new(device, layouts.frame, layouts.points)?;
        let instances = InstancePipelines::new(device, layouts.frame, layouts.instances)?;
        let relations = RelationPipelines::new(device, layouts.frame, layouts.relations)?;
        let timeline_shader = shader(
            device,
            "generic instance timeline",
            pdviewx_shaders::INSTANCE_TIMELINE,
        )?;
        let instance_timeline = pipeline(
            device,
            &timeline_shader,
            &[Some(layouts.instance_timeline)],
            "materialize generic instance timeline",
            "materialize_instance_timeline",
        )?;
        let point_timeline_shader = shader(
            device,
            "generic point timeline",
            pdviewx_shaders::POINT_TIMELINE,
        )?;
        let point_timeline = pipeline(
            device,
            &point_timeline_shader,
            &[Some(layouts.instance_timeline)],
            "materialize generic point timeline",
            "materialize_point_timeline",
        )?;
        let attribute_shader = shader(
            device,
            "attribute timeline",
            pdviewx_shaders::ATTRIBUTE_TIMELINE,
        )?;
        let attribute_timeline = pipeline(
            device,
            &attribute_shader,
            &[Some(layouts.attribute_timeline)],
            "materialize attribute timeline",
            "materialize_attribute_timeline",
        )?;
        Ok(Self {
            reset: base.reset,
            reset_tiles: base.reset_tiles,
            bin_atoms: base.bin_atoms,
            compact_tiles: base.compact_tiles,
            atoms: base.atoms,
            bonds: base.bonds,
            direct_bonds: base.direct_bonds,
            visual_cull: base.visual_cull,
            visual_shading: base.visual_shading,
            visual_shading_all: base.visual_shading_all,
            paged_reset: paged.reset,
            paged_trajectory: paged.trajectory,
            paged_cull: paged.cull,
            paged_bond_reset: paged.bond_reset,
            paged_bond_cull: paged.bond_cull,
            generic_point_reset: points.reset,
            generic_point_cull: points.cull,
            generic_point_compact: points.compact,
            generic_point_visual_cull: points.visual_cull,
            generic_point_visual_shading: points.visual_shading,
            generic_instance_reset: instances.reset,
            generic_instance_cull: instances.cull,
            generic_instance_visual_cull: instances.visual_cull,
            generic_instance_visual_shading: instances.visual_shading,
            relation_reset: relations.reset,
            relation_cull: relations.cull,
            point_timeline,
            instance_timeline,
            attribute_timeline,
        })
    }

    pub fn record(ctx: &mut PassContext<'_, D>) {
        let scene = ctx.scene;
        let cull = &ctx.passes.cull;
        let mut pass = ctx.encoder.begin_compute_pass(&ComputePassDesc {
            label: "visibility culling",
            timestamps: ctx.timestamps,
        });
        record_structures(&mut pass, scene, cull);
        record_points(&mut pass, scene, cull);
        record_instances(&mut pass, scene, cull);
        record_relations(&mut pass, scene, cull);
        record_paged(&mut pass, scene, cull);
    }
}

include!("cull_timeline.rs");
include!("cull_base.rs");

struct PagedPipelines<D: Device> {
    reset: D::Pipeline,
    trajectory: D::Pipeline,
    cull: D::Pipeline,
    bond_reset: D::Pipeline,
    bond_cull: D::Pipeline,
}

impl<D: Device> PagedPipelines<D> {
    fn new(
        device: &D,
        frame: &D::BindGroupLayout,
        chunks: &D::BindGroupLayout,
        bonds: &D::BindGroupLayout,
    ) -> Result<Self, RenderError> {
        let chunk_shader = shader(
            device,
            "paged structure chunks",
            pdviewx_shaders::PAGED_CHUNK,
        )?;
        let bond_shader = shader(device, "paged provider bonds", pdviewx_shaders::PAGED_BOND)?;
        let chunk_layouts = &[Some(frame), Some(chunks)];
        let bond_layouts = &[Some(frame), Some(bonds)];
        Ok(Self {
            reset: pipeline(
                device,
                &chunk_shader,
                chunk_layouts,
                "reset paged chunks",
                "reset_paged_chunks",
            )?,
            trajectory: pipeline(
                device,
                &chunk_shader,
                chunk_layouts,
                "interpolate paged trajectory",
                "interpolate_paged_trajectory",
            )?,
            cull: pipeline(
                device,
                &chunk_shader,
                chunk_layouts,
                "cull paged chunks",
                "cull_paged_chunks",
            )?,
            bond_reset: pipeline(
                device,
                &bond_shader,
                bond_layouts,
                "reset paged bonds",
                "reset_paged_bonds",
            )?,
            bond_cull: pipeline(
                device,
                &bond_shader,
                bond_layouts,
                "cull paged bonds",
                "cull_paged_bonds",
            )?,
        })
    }
}

struct PointPipelines<D: Device> {
    reset: D::Pipeline,
    cull: D::Pipeline,
    compact: D::Pipeline,
    visual_cull: D::Pipeline,
    visual_shading: D::Pipeline,
}

impl<D: Device> PointPipelines<D> {
    fn new(
        device: &D,
        frame: &D::BindGroupLayout,
        points: &D::BindGroupLayout,
    ) -> Result<Self, RenderError> {
        let shader = shader(
            device,
            "generic point culling",
            pdviewx_shaders::GENERIC_POINT_CULL,
        )?;
        let layouts = &[Some(frame), Some(points)];
        Ok(Self {
            reset: pipeline(
                device,
                &shader,
                layouts,
                "reset generic points",
                "reset_generic_points",
            )?,
            cull: pipeline(
                device,
                &shader,
                layouts,
                "cull generic points",
                "bin_generic_points",
            )?,
            compact: pipeline(
                device,
                &shader,
                layouts,
                "compact generic points",
                "compact_generic_point_tiles",
            )?,
            visual_cull: pipeline(
                device,
                &shader,
                layouts,
                "evaluate point cull visuals",
                "evaluate_generic_point_visual_cull",
            )?,
            visual_shading: pipeline(
                device,
                &shader,
                layouts,
                "evaluate point appearance",
                "evaluate_generic_point_visual_shading",
            )?,
        })
    }
}

struct InstancePipelines<D: Device> {
    reset: D::Pipeline,
    cull: D::Pipeline,
    visual_cull: D::Pipeline,
    visual_shading: D::Pipeline,
}

impl<D: Device> InstancePipelines<D> {
    fn new(
        device: &D,
        frame: &D::BindGroupLayout,
        instances: &D::BindGroupLayout,
    ) -> Result<Self, RenderError> {
        let shader = shader(
            device,
            "generic instance culling",
            pdviewx_shaders::GENERIC_INSTANCE_CULL,
        )?;
        let layouts = &[Some(frame), Some(instances)];
        Ok(Self {
            reset: pipeline(
                device,
                &shader,
                layouts,
                "reset generic instances",
                "reset_generic_instances",
            )?,
            cull: pipeline(
                device,
                &shader,
                layouts,
                "cull generic instances",
                "cull_generic_instances",
            )?,
            visual_cull: pipeline(
                device,
                &shader,
                layouts,
                "evaluate instance cull visuals",
                "evaluate_generic_instance_visual_cull",
            )?,
            visual_shading: pipeline(
                device,
                &shader,
                layouts,
                "evaluate instance appearance",
                "evaluate_generic_instance_visual_shading",
            )?,
        })
    }
}

include!("cull_relations.rs");
include!("cull_pipeline.rs");

fn record_structures<D: Device, P: ComputePassEncoder<D>>(
    pass: &mut P,
    scene: &crate::scene_gpu::GpuScene<D>,
    cull: &CullPass<D>,
) {
    for dispatch in scene.cull_dispatches() {
        if dispatch.modes.cull_visual() && dispatch.visual_groups[0] > 0 {
            pass.set_bind_group(0, dispatch.visual_group, &[]);
            pass.set_pipeline(&cull.visual_cull);
            pass.dispatch(dispatch.visual_groups[0], dispatch.visual_groups[1], 1);
        }
        pass.set_bind_group(0, dispatch.atom_group, &[]);
        pass.set_pipeline(&cull.reset);
        pass.dispatch(1, 1, 1);
        if dispatch.atom_groups[0] > 0 {
            if dispatch.modes.lod() {
                pass.set_pipeline(&cull.reset_tiles);
                pass.dispatch(dispatch.tile_groups[0], dispatch.tile_groups[1], 1);
                pass.set_pipeline(&cull.bin_atoms);
                let groups = if dispatch.modes.fast_points() {
                    dispatch.bin_groups
                } else {
                    dispatch.atom_groups
                };
                pass.dispatch(groups[0], groups[1], 1);
            }
            if dispatch.modes.fast_points() {
                pass.set_pipeline(&cull.compact_tiles);
                pass.dispatch(dispatch.tile_groups[0], dispatch.tile_groups[1], 1);
            } else {
                pass.set_pipeline(&cull.atoms);
                pass.dispatch(dispatch.atom_groups[0], dispatch.atom_groups[1], 1);
            }
        }
        if dispatch.bond_groups[0] > 0 {
            pass.set_bind_group(0, dispatch.bond_group, &[]);
            pass.set_pipeline(if dispatch.modes.direct_bonds() {
                &cull.direct_bonds
            } else {
                &cull.bonds
            });
            pass.dispatch(dispatch.bond_groups[0], dispatch.bond_groups[1], 1);
        }
        if dispatch.modes.shading_visual() && dispatch.visual_groups[0] > 0 {
            pass.set_bind_group(0, dispatch.visual_group, &[]);
            pass.set_pipeline(if dispatch.modes.shading_all() {
                &cull.visual_shading_all
            } else {
                &cull.visual_shading
            });
            pass.dispatch(dispatch.visual_groups[0], dispatch.visual_groups[1], 1);
        }
    }
}

fn record_points<D: Device, P: ComputePassEncoder<D>>(
    pass: &mut P,
    scene: &crate::scene_gpu::GpuScene<D>,
    cull: &CullPass<D>,
) {
    for dispatch in scene.generic_point_dispatches() {
        pass.set_bind_group(0, &scene.group0, &[]);
        pass.set_bind_group(1, dispatch.group, &[]);
        if dispatch.cull_visual {
            pass.set_pipeline(&cull.generic_point_visual_cull);
            pass.dispatch(dispatch.source_groups[0], dispatch.source_groups[1], 1);
        }
        pass.set_pipeline(&cull.generic_point_reset);
        pass.dispatch(dispatch.tile_groups[0], dispatch.tile_groups[1], 1);
        pass.set_pipeline(&cull.generic_point_cull);
        pass.dispatch(dispatch.source_groups[0], dispatch.source_groups[1], 1);
        pass.set_pipeline(&cull.generic_point_compact);
        pass.dispatch(dispatch.tile_groups[0], dispatch.tile_groups[1], 1);
        if dispatch.shading_visual {
            pass.set_pipeline(&cull.generic_point_visual_shading);
            pass.dispatch(dispatch.source_groups[0], dispatch.source_groups[1], 1);
        }
    }
}

fn record_instances<D: Device, P: ComputePassEncoder<D>>(
    pass: &mut P,
    scene: &crate::scene_gpu::GpuScene<D>,
    cull: &CullPass<D>,
) {
    for dispatch in scene.generic_instance_dispatches() {
        pass.set_bind_group(0, &scene.group0, &[]);
        pass.set_bind_group(1, dispatch.group, &[]);
        if dispatch.cull_visual {
            pass.set_pipeline(&cull.generic_instance_visual_cull);
            pass.dispatch(dispatch.groups[0], dispatch.groups[1], 1);
        }
        pass.set_pipeline(&cull.generic_instance_reset);
        pass.dispatch(1, 1, 1);
        pass.set_pipeline(&cull.generic_instance_cull);
        pass.dispatch(dispatch.groups[0], dispatch.groups[1], 1);
        if dispatch.shading_visual {
            pass.set_pipeline(&cull.generic_instance_visual_shading);
            pass.dispatch(dispatch.groups[0], dispatch.groups[1], 1);
        }
    }
}

include!("cull_paged.rs");
