//! Allocation-stable resolution of scene-tracked optical focus targets.
//!
//! A selection centroid is reduced only when its membership, placement,
//! coordinate generation, or resident trajectory pair changes. During
//! trajectory playback, endpoint centroids are interpolated in `O(structures)`;
//! no atom rows are scanned and no coordinates are materialized per frame.

use super::{Engine, FocusTarget};
use crate::error::RenderError;
use molgfx_core::{AtomSelection, Scene, SelectionHandle, StructureHandle};
use molgfx_gpu::Device;
use molgfx_math::{Camera, Mat4, Vec3};

#[cfg(test)]
#[path = "focus_target_tests.rs"]
mod tests;

#[derive(Clone, Copy, PartialEq, Debug)]
struct StructureFocus {
    structure: StructureHandle,
    transform: [u32; 16],
    coordinate_generation: u64,
    trajectory_pair_revision: u64,
    count: u32,
    start: Vec3,
    end: Vec3,
}

/// Cached selection reductions shared by surface and off-screen frame paths.
#[derive(Debug, Default)]
pub(crate) struct FocusTracker {
    selection: Option<SelectionHandle>,
    structures: Vec<StructureFocus>,
}

impl FocusTracker {
    pub(crate) fn resolve(
        &mut self,
        target: FocusTarget,
        scene: &Scene,
        camera: &Camera,
    ) -> Result<f32, RenderError> {
        match target {
            FocusTarget::CameraTarget => Ok(camera.focus_distance().max(1.0e-3)),
            FocusTarget::Distance(distance) => Ok(distance.max(1.0e-3)),
            FocusTarget::WorldPoint(point) => view_distance(camera, point),
            FocusTarget::Selection(selection) => {
                let point = self.selection_centroid(selection, scene)?;
                view_distance(camera, point)
            }
        }
    }

    fn selection_centroid(
        &mut self,
        selection: SelectionHandle,
        scene: &Scene,
    ) -> Result<Vec3, RenderError> {
        if self.selection != Some(selection) {
            self.selection = Some(selection);
            self.structures.clear();
        }
        let mut entry_index = 0;
        let mut centroid = Vec3::ZERO;
        let mut total = 0_u64;
        for (structure, placed) in scene.structures() {
            let Some(rows) = scene.selection_for(selection, structure) else {
                continue;
            };
            let count = rows.count(placed.atoms.len());
            if count == 0 {
                continue;
            }
            let signature = StructureFocus {
                structure,
                transform: transform_bits(placed.model_to_world),
                coordinate_generation: placed.atoms.coords().generation(),
                trajectory_pair_revision: placed.trajectory_pair_revision(),
                count: 0,
                start: Vec3::ZERO,
                end: Vec3::ZERO,
            };
            let cached = self.structures.get(entry_index).copied();
            let entry = match cached {
                Some(entry) if same_source(entry, signature) => entry,
                _ => reduce_structure(structure, placed, rows, signature)?,
            };
            match self.structures.get_mut(entry_index) {
                Some(slot) => *slot = entry,
                None => self.structures.push(entry),
            }
            let alpha = placed
                .trajectory()
                .map_or(0.0, molgfx_core::TrajectorySegment::interpolation);
            let current = entry.start.lerp(entry.end, alpha);
            let combined = total.saturating_add(u64::from(entry.count));
            let weight = integer_as_f32(u64::from(entry.count)) / integer_as_f32(combined);
            centroid = centroid.lerp(current, weight);
            total = combined;
            entry_index += 1;
        }
        self.structures.truncate(entry_index);
        if total == 0 {
            return Err(RenderError::InvalidFocusTarget {
                reason: "selection is stale or empty",
            });
        }
        Ok(centroid)
    }
}

impl<D: Device> Engine<D> {
    pub(super) fn resolve_optics(
        &mut self,
        scene: &Scene,
        camera: &Camera,
    ) -> Result<[f32; 4], RenderError> {
        let plan = self.resolved_plan;
        let target = plan
            .depth_of_field()
            .map_or(FocusTarget::CameraTarget, |settings| settings.focus);
        let distance = self.focus_tracker.resolve(target, scene, camera)?;
        Ok(plan.optics(distance))
    }
}

fn same_source(left: StructureFocus, right: StructureFocus) -> bool {
    left.structure == right.structure
        && left.transform == right.transform
        && left.coordinate_generation == right.coordinate_generation
        && left.trajectory_pair_revision == right.trajectory_pair_revision
}

fn reduce_structure(
    structure: StructureHandle,
    placed: &molgfx_core::PlacedStructure,
    rows: &AtomSelection,
    signature: StructureFocus,
) -> Result<StructureFocus, RenderError> {
    let (start, end) = if let Some(segment) = placed.trajectory() {
        (segment.start().positions(), segment.end().positions())
    } else {
        let positions = placed.atoms.coords().slice();
        (positions, positions)
    };
    let mut start_sum = [0.0; 3];
    let mut start_error = [0.0; 3];
    let mut end_sum = [0.0; 3];
    let mut end_error = [0.0; 3];
    let mut count = 0_u32;
    rows.for_each(placed.atoms.len(), |row| {
        let index = row as usize;
        let (Some(a), Some(b)) = (start.get(index), end.get(index)) else {
            return;
        };
        for lane in 0..3 {
            compensated_add(&mut start_sum[lane], &mut start_error[lane], a[lane]);
            compensated_add(&mut end_sum[lane], &mut end_error[lane], b[lane]);
        }
        count = count.saturating_add(1);
    });
    if count == 0 {
        return Err(RenderError::InvalidFocusTarget {
            reason: "selection is empty",
        });
    }
    let divisor = integer_as_f32(u64::from(count));
    let local_start = Vec3::new(
        start_sum[0] / divisor,
        start_sum[1] / divisor,
        start_sum[2] / divisor,
    );
    let local_end = Vec3::new(
        end_sum[0] / divisor,
        end_sum[1] / divisor,
        end_sum[2] / divisor,
    );
    Ok(StructureFocus {
        structure,
        count,
        start: placed.model_to_world.transform_point3(local_start),
        end: placed.model_to_world.transform_point3(local_end),
        ..signature
    })
}

fn compensated_add(sum: &mut f32, error: &mut f32, value: f32) {
    let corrected = value - *error;
    let next = *sum + corrected;
    *error = (next - *sum) - corrected;
    *sum = next;
}

fn integer_as_f32(value: u64) -> f32 {
    [48, 32, 16, 0].into_iter().fold(0.0, |result, shift| {
        let limb = crate::fallback(u16::try_from((value >> shift) & u64::from(u16::MAX)), 0);
        result * 65_536.0 + f32::from(limb)
    })
}

fn view_distance(camera: &Camera, point: Vec3) -> Result<f32, RenderError> {
    let forward = (camera.target - camera.eye).normalize_or_zero();
    let distance = (point - camera.eye).dot(forward);
    if !distance.is_finite() || distance <= 0.0 {
        return Err(RenderError::InvalidFocusTarget {
            reason: "target must lie in front of the camera",
        });
    }
    Ok(distance)
}

fn transform_bits(transform: Mat4) -> [u32; 16] {
    transform.to_cols_array().map(f32::to_bits)
}
