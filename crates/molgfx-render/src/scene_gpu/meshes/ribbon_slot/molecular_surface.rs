//! Analytical provider geometry validation before GPU packing.
use super::RibbonSync;
use crate::error::RenderError;
use molgfx_gpu::Device;
use molgfx_math::Rgba8;

pub(super) fn prepare<D: Device>(
    input: &mut RibbonSync<'_, D>,
    color: Rgba8,
) -> Result<(), RenderError> {
    let surface = input.representation.molecular_surface.as_ref().ok_or(
        RenderError::SourceCapabilityMissing {
            capability: "analytical molecular surface geometry",
        },
    )?;
    surface.validate_pose(input.placed)?;
    if !matches!(
        input.representation.params.surface_style,
        molgfx_core::SurfaceStyle::Solid | molgfx_core::SurfaceStyle::Mesh
    ) {
        return Err(molgfx_core::CoreError::InvalidMesh {
            reason: "analytical molecular surfaces support solid or triangle wire presentation",
        }
        .into());
    }
    input.mesh.generate_molecular_surface(
        surface,
        input.selection,
        input.representation.params.surface_style == molgfx_core::SurfaceStyle::Mesh,
        color,
    )?;
    Ok(())
}
