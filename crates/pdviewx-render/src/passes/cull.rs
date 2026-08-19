//! GPU visibility compaction and indirect argument generation.

use crate::error::RenderError;
use crate::graph::PassContext;
use pdviewx_gpu::{
    CommandEncoder as _, ComputePassDesc, ComputePassEncoder as _, ComputePipelineDesc, Device,
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
}

impl<D: Device> CullPass<D> {
    pub fn new(device: &D, layout: &D::BindGroupLayout) -> Result<Self, RenderError> {
        let shader = device.create_shader_module(&ShaderModuleDesc {
            label: "cull",
            wgsl: pdviewx_shaders::CULL,
        })?;
        let pipeline = |label, entry| {
            device.create_compute_pipeline(&ComputePipelineDesc {
                label,
                layouts: &[Some(layout)],
                shader: &shader,
                entry,
            })
        };
        Ok(Self {
            reset: pipeline("reset indirect arguments", "reset_cull")?,
            reset_tiles: pipeline("reset screen tile visibility", "reset_tiles")?,
            bin_atoms: pipeline("bin nearest screen tile atoms", "bin_atoms")?,
            compact_tiles: pipeline("compact screen tile atoms", "compact_tiles")?,
            atoms: pipeline("compact visible atoms", "cull_atoms")?,
            bonds: pipeline("compact visible bonds", "cull_bonds")?,
            direct_bonds: pipeline("compact visible wire bonds", "cull_bonds_direct")?,
        })
    }

    pub fn record(ctx: &mut PassContext<'_, D>) {
        let mut pass = ctx.encoder.begin_compute_pass(&ComputePassDesc {
            label: "visibility culling",
            timestamps: ctx.timestamps,
        });
        for dispatch in ctx.scene.cull_dispatches() {
            pass.set_bind_group(0, dispatch.group, &[]);
            pass.set_pipeline(&ctx.passes.cull.reset);
            pass.dispatch(1, 1, 1);
            if dispatch.atom_groups > 0 {
                if dispatch.lod {
                    pass.set_pipeline(&ctx.passes.cull.reset_tiles);
                    pass.dispatch(dispatch.tile_groups, 1, 1);
                    pass.set_pipeline(&ctx.passes.cull.bin_atoms);
                    pass.dispatch(
                        if dispatch.fast_points {
                            dispatch.bin_groups
                        } else {
                            dispatch.atom_groups
                        },
                        1,
                        1,
                    );
                }
                if dispatch.fast_points {
                    pass.set_pipeline(&ctx.passes.cull.compact_tiles);
                    pass.dispatch(dispatch.tile_groups, 1, 1);
                } else {
                    pass.set_pipeline(&ctx.passes.cull.atoms);
                    pass.dispatch(dispatch.atom_groups, 1, 1);
                }
            }
            if dispatch.bond_groups > 0 {
                pass.set_pipeline(if dispatch.direct_bonds {
                    &ctx.passes.cull.direct_bonds
                } else {
                    &ctx.passes.cull.bonds
                });
                pass.dispatch(dispatch.bond_groups, 1, 1);
            }
        }
    }
}
