use super::pipeline::{pipeline, shader};
use crate::error::RenderError;
use molgfx_gpu::Device;

pub(super) struct BasePipelines<D: Device> {
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
}

impl<D: Device> BasePipelines<D> {
    pub(super) fn new(
        device: &D,
        atom_layout: &D::BindGroupLayout,
        bond_layout: &D::BindGroupLayout,
        visual_layout: &D::BindGroupLayout,
    ) -> Result<Self, RenderError> {
        let cull = shader(device, "cull", molgfx_shaders::CULL)?;
        let visual = shader(
            device,
            "visual program evaluator",
            molgfx_shaders::VISUAL_PROGRAM,
        )?;
        let atom_layouts = &[Some(atom_layout)];
        let bond_layouts = &[Some(bond_layout)];
        let visual_layouts = &[Some(visual_layout)];
        Ok(Self {
            reset: pipeline(
                device,
                &cull,
                atom_layouts,
                "reset indirect arguments",
                "reset_cull",
            )?,
            reset_tiles: pipeline(
                device,
                &cull,
                atom_layouts,
                "reset screen tiles",
                "reset_tiles",
            )?,
            bin_atoms: pipeline(
                device,
                &cull,
                atom_layouts,
                "bin screen tile atoms",
                "bin_atoms",
            )?,
            compact_tiles: pipeline(
                device,
                &cull,
                atom_layouts,
                "compact screen tiles",
                "compact_tiles",
            )?,
            atoms: pipeline(device, &cull, atom_layouts, "compact atoms", "cull_atoms")?,
            bonds: pipeline(device, &cull, bond_layouts, "compact bonds", "cull_bonds")?,
            direct_bonds: pipeline(
                device,
                &cull,
                bond_layouts,
                "compact wire bonds",
                "cull_bonds_direct",
            )?,
            visual_cull: pipeline(
                device,
                &visual,
                visual_layouts,
                "evaluate cull visuals",
                "evaluate_visual_cull",
            )?,
            visual_shading: pipeline(
                device,
                &visual,
                visual_layouts,
                "evaluate compact visuals",
                "evaluate_visual_shading",
            )?,
            visual_shading_all: pipeline(
                device,
                &visual,
                visual_layouts,
                "evaluate all visuals",
                "evaluate_visual_shading_all",
            )?,
        })
    }
}
