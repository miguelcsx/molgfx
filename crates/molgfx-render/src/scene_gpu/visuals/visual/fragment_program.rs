//! Fixed-size fragment-only visual program upload record.

use molgfx_core::{MAX_VISUAL_INSTRUCTIONS, MAX_VISUAL_PARAMETERS, VisualInstructionGpu};

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, bytemuck::Pod, bytemuck::Zeroable)]
pub(super) struct VisualFragmentProgram {
    pub(super) instructions: [VisualInstructionGpu; MAX_VISUAL_INSTRUCTIONS],
    pub(super) parameters: [[f32; 4]; MAX_VISUAL_PARAMETERS],
}

pub(super) const FRAGMENT_PARAMETER_OFFSET: u64 =
    std::mem::size_of::<[VisualInstructionGpu; MAX_VISUAL_INSTRUCTIONS]>() as u64;

impl Default for VisualFragmentProgram {
    fn default() -> Self {
        const EMPTY: VisualInstructionGpu = VisualInstructionGpu {
            control: [0; 4],
            data: [0.0; 4],
        };
        Self {
            instructions: [EMPTY; MAX_VISUAL_INSTRUCTIONS],
            parameters: [[0.0; 4]; MAX_VISUAL_PARAMETERS],
        }
    }
}
