//! Draw recording for the specialized transparent geometry.
//!
//! Draws arrive grouped by representation, so each helper changes pipeline at
//! most once per representation rather than once per draw.

use crate::passes::PassRegistry;
use crate::scene_gpu::GpuScene;
use pdviewx_gpu::{Device, RenderPassEncoder};

/// Records transparent spheres, switching pipeline only when the clip state
/// changes between representations.
pub(super) fn record_spheres<D: Device, P: RenderPassEncoder<D>>(
    passes: &PassRegistry<D>,
    scene: &GpuScene<D>,
    pass: &mut P,
) {
    let mut bound = None;
    for (group, args, shading) in scene.atom_draws(true) {
        if bound != Some(shading.clipped) {
            pass.set_pipeline(if shading.clipped {
                &passes.oit.sphere_clipped
            } else {
                &passes.oit.sphere
            });
            bound = Some(shading.clipped);
        }
        pass.set_bind_group(2, group, &[]);
        pass.draw_indirect(args, 0);
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
    for (group, args, shading) in scene.surface_draws(true) {
        if bound != Some(shading.surface_grid) {
            pass.set_pipeline(if shading.surface_grid {
                &passes.oit.grid_surface
            } else {
                &passes.oit.union_surface
            });
            bound = Some(shading.surface_grid);
        }
        pass.set_bind_group(2, group, &[]);
        pass.draw_indirect(args, 0);
    }
}

/// Records transparent primitives, changing pipeline at most once per class as
/// it walks the shape-sorted table.
pub(super) fn record_primitives<D: Device, P: RenderPassEncoder<D>>(
    passes: &PassRegistry<D>,
    scene: &GpuScene<D>,
    pass: &mut P,
) {
    let Some((table, runs)) = scene.primitive_groups() else {
        return;
    };
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
