//! GPU visibility compaction and indirect argument generation.

use crate::error::RenderError;
use crate::graph::PassContext;
use molgfx_gpu::{CommandEncoder as _, ComputePassDesc, ComputePassEncoder, Device};

use super::paged::record_paged;
use super::pipeline::{pipeline, shader};
use super::relations::{RelationPipelines, record_relations};
use super::structure_pipelines::BasePipelines;

#[derive(Debug)]
pub(crate) struct CullPass<D: Device> {
    pub(super) reset: D::Pipeline,
    pub(super) reset_tiles: D::Pipeline,
    pub(super) bin_atoms: D::Pipeline,
    pub(super) compact_tiles: D::Pipeline,
    pub(super) atoms: D::Pipeline,
    pub(super) bonds: D::Pipeline,
    pub(super) direct_bonds: D::Pipeline,
    pub(super) visual_cull: D::Pipeline,
    pub(super) visual_shading: D::Pipeline,
    pub(super) visual_shading_all: D::Pipeline,
    pub(super) paged_reset: D::Pipeline,
    pub(super) paged_trajectory: D::Pipeline,
    pub(super) paged_cull: D::Pipeline,
    pub(super) paged_bond_reset: D::Pipeline,
    pub(super) paged_bond_cull: D::Pipeline,
    pub(super) generic_point_reset: D::Pipeline,
    pub(super) generic_point_cull: D::Pipeline,
    pub(super) generic_point_compact: D::Pipeline,
    pub(super) generic_point_visual_cull: D::Pipeline,
    pub(super) generic_point_visual_shading: D::Pipeline,
    pub(super) generic_instance_reset: D::Pipeline,
    pub(super) generic_instance_cull: D::Pipeline,
    pub(super) generic_instance_visual_cull: D::Pipeline,
    pub(super) generic_instance_visual_shading: D::Pipeline,
    pub(super) relation_reset: D::Pipeline,
    pub(super) relation_cull: D::Pipeline,
    pub(super) relation_scan: D::Pipeline,
    pub(super) relation_prefix: D::Pipeline,
    pub(super) relation_scatter: D::Pipeline,
    pub(super) point_timeline: D::Pipeline,
    pub(super) instance_timeline: D::Pipeline,
    pub(super) attribute_timeline: D::Pipeline,
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
            molgfx_shaders::INSTANCE_TIMELINE,
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
            molgfx_shaders::POINT_TIMELINE,
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
            molgfx_shaders::ATTRIBUTE_TIMELINE,
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
            relation_scan: relations.scan,
            relation_prefix: relations.prefix,
            relation_scatter: relations.scatter,
            point_timeline,
            instance_timeline,
            attribute_timeline,
        })
    }

    pub(crate) fn record(ctx: &mut PassContext<'_, D>) {
        let scene = ctx.scene;
        let cull = &ctx.passes.cull;
        // An empty compute pass leaves its timestamps unresolved, which voids
        // the frame's GPU timing, so a sample with nothing to cull has no pass.
        if ctx.timestamps.is_none() && reuses_whole_cull(scene) {
            return;
        }
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

/// Whether this sample's culling is entirely the standing occlusion cull.
fn reuses_whole_cull<D: Device>(scene: &crate::scene_gpu::GpuScene<D>) -> bool {
    scene.reuses_cull_view()
        && scene
            .cull_dispatches()
            .all(|dispatch| dispatch.modes.view_reusable())
        && scene.generic_point_dispatches().next().is_none()
        && scene.generic_instance_dispatches().next().is_none()
        && scene.relation_cull_dispatches().next().is_none()
        && scene.paged_chunk_cull().is_none()
        && scene.paged_bond_cull().is_none()
}

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
            molgfx_shaders::PAGED_CHUNK,
        )?;
        let bond_shader = shader(device, "paged provider bonds", molgfx_shaders::PAGED_BOND)?;
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
            molgfx_shaders::GENERIC_POINT_CULL,
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
            molgfx_shaders::GENERIC_INSTANCE_CULL,
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

fn record_structures<D: Device, P: ComputePassEncoder<D>>(
    pass: &mut P,
    scene: &crate::scene_gpu::GpuScene<D>,
    cull: &CullPass<D>,
) {
    let reuse = scene.reuses_cull_view();
    for dispatch in scene.cull_dispatches() {
        if dispatch.modes.view_reusable() {
            // Samples of one converged view differ by under a pixel, which the
            // cull already pads, so the visible list of the first one stands.
            if reuse {
                continue;
            }
            scene.mark_cull_view_recorded();
        }
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
                // The depth planes hold millions of words, so the clear is
                // spread over as many groups as there are atoms to bin.
                let clear = if dispatch.modes.occlusion() {
                    dispatch.atom_groups
                } else {
                    dispatch.tile_groups
                };
                pass.dispatch(clear[0], clear[1], 1);
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
