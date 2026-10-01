//! Lowering crystallographic and caller-authored guide geometry.

use crate::error::Error;
use crate::id::StructureId;
use molgfx_core::{CrystalCell, GuideHandle, GuideStyle, PlanarRegion, Scene, StructureHandle};
use molgfx_math::Vec3;
use std::collections::BTreeMap;

/// Emits one twelve-edge unit cell for every declared structure owner.
///
/// Cost: `O(structures)` CPU work and twelve guide inserts per owner.
pub(crate) fn lower_unit_cells(
    spec: &crate::SceneSpec,
    scene: &mut Scene,
    handles: &BTreeMap<StructureId, StructureHandle>,
) -> Result<Vec<(StructureId, Vec<GuideHandle>)>, Error> {
    let Some(assembly) = &spec.assembly else {
        return Ok(Vec::new());
    };
    let Some(cell) = assembly.unit_cell else {
        return Ok(Vec::new());
    };
    let native = CrystalCell::new(cell.lengths, cell.angles_degrees)?
        .with_origin(Vec3::from_array(cell.origin))?;
    let style = GuideStyle::default();
    let mut lowered = Vec::with_capacity(assembly.structures.len());
    for &structure in &assembly.structures {
        let owner = handles.get(&structure).copied().ok_or_else(|| {
            Error::InvalidSpec("unit cell targets an unbound structure".to_owned())
        })?;
        lowered.push((structure, scene.add_unit_cell(owner, native, style)?));
    }
    Ok(lowered)
}

/// Emits four guide segments per caller-authored plane.
///
/// Cost: `O(planes)` CPU work and four guide inserts per plane.
pub(crate) fn lower_planes(
    spec: &crate::SceneSpec,
    scene: &mut Scene,
    handles: &BTreeMap<StructureId, StructureHandle>,
) -> Result<Vec<(crate::PlaneId, Vec<GuideHandle>)>, Error> {
    let mut lowered = Vec::with_capacity(spec.planes.len());
    for (id, plane) in &spec.planes {
        let owner = handles
            .get(&plane.structure)
            .copied()
            .ok_or_else(|| Error::InvalidSpec("plane targets an unbound structure".to_owned()))?;
        let region = PlanarRegion::new(
            owner,
            Vec3::from_array(plane.center),
            Vec3::from_array(plane.normal),
            Vec3::from_array(plane.tangent),
            plane.size,
        )?;
        let style = GuideStyle {
            color: plane.color.native(),
            width_pixels: plane.width_pixels,
            opacity: plane.opacity,
            ..GuideStyle::default()
        };
        lowered.push((*id, scene.add_planar_region(region, style)?));
    }
    Ok(lowered)
}
