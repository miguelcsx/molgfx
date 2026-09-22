//! Draw recording for the specialized transparent geometry.
//!
//! Draws arrive grouped by representation, so each helper changes pipeline at
//! most once per representation rather than once per draw.

use crate::passes::PassRegistry;
use crate::scene_gpu::GpuScene;
use molgfx_gpu::{Device, RenderPassEncoder};

/// Records transparent spheres, switching pipeline only when the clip state
/// changes between representations.
pub(super) fn record_spheres<D: Device, P: RenderPassEncoder<D>>(
    passes: &PassRegistry<D>,
    scene: &GpuScene<D>,
    pass: &mut P,
) {
    let mut bound = None;
    if let Some(arena) = scene.indirect_args() {
        for (group, offset, shading, _) in scene.atom_draws(true) {
            if bound != Some(shading) {
                pass.set_pipeline(if shading.clipped() {
                    passes.oit.sphere_clipped.get(shading)
                } else {
                    passes.oit.sphere.get(shading)
                });
                bound = Some(shading);
            }
            pass.set_bind_group(2, group, &[]);
            pass.draw_indirect(arena, offset);
        }
    }
}

/// Records transparent surfaces, switching between the analytic union and the
/// persistent-grid tracing only when the surface family changes.
pub(super) fn record_surfaces<D: Device, P: RenderPassEncoder<D>>(
    passes: &PassRegistry<D>,
    scene: &GpuScene<D>,
    pass: &mut P,
) {
    let mut bound = None;
    if let Some(arena) = scene.indirect_args() {
        for (group, offset, shading, _) in scene.surface_draws(true) {
            if bound != Some(shading) {
                pass.set_pipeline(if shading.surface_grid() {
                    passes.oit.grid_surface.get(shading)
                } else {
                    passes.oit.union_surface.get(shading)
                });
                bound = Some(shading);
            }
            pass.set_bind_group(2, group, &[]);
            pass.draw_indirect(arena, offset);
        }
    }
}

/// Records transparent primitives, changing pipeline at most once per class as
/// it walks the shape-sorted table.
pub(super) fn record_primitives<D: Device, P: RenderPassEncoder<D>>(
    passes: &PassRegistry<D>,
    scene: &GpuScene<D>,
    pass: &mut P,
) {
    if let Some((table, runs)) = scene.primitive_groups() {
        pass.set_bind_group(2, table, &[]);
        for run in runs.iter().filter(|run| run.translucent) {
            let Some(pipeline) = passes.oit.primitive.pipeline(run) else {
                continue;
            };
            pass.set_pipeline(pipeline);
            pass.draw(
                0..crate::passes::primitive_pipelines::PRIMITIVE_QUAD_VERTICES,
                run.first..run.first + run.len,
            );
        }
    }
    if let Some((table, args, runs)) = scene.ligand_pose_draws() {
        pass.set_bind_group(2, table, &[]);
        for run in runs.iter().filter(|run| run.translucent) {
            let Some(pipeline) = passes.oit.ligand_pose.pipeline(run) else {
                continue;
            };
            pass.set_pipeline(pipeline);
            pass.draw_indirect(args, run.args_offset);
        }
    }
}
