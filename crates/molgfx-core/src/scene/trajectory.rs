//! Scene editing for bounded, caller-supplied trajectory presentation.

use crate::{
    AtomProperty, AtomPropertyMeaning, CoreError, Guide, GuideCap, GuideHandle, GuideStyle,
    ScalarFieldSemantics, Scene, StructureHandle, TrajectorySegment,
};
use molgfx_math::Vec3;
use std::sync::Arc;

#[cfg(test)]
#[path = "trajectory_tests.rs"]
mod tests;

impl Scene {
    /// Replaces the two resident frames for one topology-stable structure.
    ///
    /// The frame arrays remain shared with the caller. A conservative BVH is
    /// rebuilt once for the pair and remains valid for every linear sample.
    ///
    /// # Errors
    ///
    /// Returns a typed error for a stale handle or mismatched atom count.
    pub fn set_trajectory_segment(
        &mut self,
        handle: StructureHandle,
        segment: TrajectorySegment,
    ) -> Result<(), CoreError> {
        let placed = self.structure_mut(handle).ok_or(CoreError::StaleHandle)?;
        if segment.atom_count() != placed.atoms.len() as usize {
            return Err(CoreError::InvalidTrajectory {
                reason: "trajectory atom count must match the placed topology",
            });
        }
        placed.replace_trajectory(segment)
    }

    /// Advances presentation time inside the resident frame interval.
    ///
    /// # Errors
    ///
    /// Returns a typed error for a stale handle, absent interval or time
    /// outside the closed resident interval.
    pub fn set_trajectory_time(
        &mut self,
        handle: StructureHandle,
        sample_seconds: f32,
    ) -> Result<(), CoreError> {
        self.structure_mut(handle)
            .ok_or(CoreError::StaleHandle)?
            .set_trajectory_time(sample_seconds)
    }

    /// Emits per-atom displacement arrows ("porcupine plot") from the resident
    /// trajectory pair.
    ///
    /// The direction and length come from `end - start` of the two frames the
    /// scene already holds — the frame-to-frame velocity — not from any
    /// renderer-side integration. Each arrow is anchored at the atom's current
    /// interpolated position, so it sits on the moving atom for the active
    /// sample time. The arrows are stored guides: advancing time does not move
    /// them, so re-call after a large advance to refresh a snapshot.
    ///
    /// `atoms` are caller-chosen model atom indices (for a classic Cα
    /// porcupine, pass the Cα indices resolved upstream). Atoms whose
    /// displacement is at or below `min_displacement` model units are skipped,
    /// which keeps near-static atoms from cluttering the field and drops the
    /// exactly-static atoms that would be a degenerate zero-length arrow. The
    /// style's cap defaults to a single arrowhead when the caller leaves it
    /// unset.
    ///
    /// # Errors
    ///
    /// Returns [`CoreError::StaleHandle`] for an absent structure,
    /// [`CoreError::InvalidTrajectory`] when the structure has no active segment
    /// or an index is out of range, or [`CoreError::InvalidAnnotation`] for a
    /// non-finite or non-positive scale, or non-finite `min_displacement`.
    pub fn add_trajectory_vectors(
        &mut self,
        handle: StructureHandle,
        atoms: &[u32],
        scale: f32,
        min_displacement: f32,
        style: GuideStyle,
    ) -> Result<Vec<GuideHandle>, CoreError> {
        if !scale.is_finite() || scale <= 0.0 {
            return Err(CoreError::InvalidAnnotation {
                reason: "trajectory vector scale must be finite and positive",
            });
        }
        if !min_displacement.is_finite() || min_displacement < 0.0 {
            return Err(CoreError::InvalidAnnotation {
                reason: "trajectory vector minimum displacement must be finite and non-negative",
            });
        }
        let placed = self.structure(handle).ok_or(CoreError::StaleHandle)?;
        let segment = placed.trajectory().ok_or(CoreError::InvalidTrajectory {
            reason: "structure has no active trajectory segment",
        })?;
        let start = segment.start().positions();
        let end = segment.end().positions();
        let alpha = segment.interpolation();
        let mut arrows = Vec::with_capacity(atoms.len());
        for &index in atoms {
            let i = index as usize;
            let (Some(from), Some(to)) = (start.get(i), end.get(i)) else {
                return Err(CoreError::InvalidTrajectory {
                    reason: "trajectory vector atom index out of range",
                });
            };
            let from = Vec3::from_array(*from);
            let displacement = Vec3::from_array(*to) - from;
            if displacement.length() <= min_displacement {
                continue;
            }
            let tail = from + displacement * alpha;
            arrows.push((tail, tail + displacement * scale));
        }
        let style = GuideStyle {
            cap: match style.cap {
                GuideCap::None => GuideCap::Arrow,
                other => other,
            },
            ..style
        }
        .sanitized();
        let mut handles = Vec::with_capacity(arrows.len());
        for (tail, head) in arrows {
            handles.push(self.add_guide(Guide::new(handle, tail, head, style)?)?);
        }
        Ok(handles)
    }

    /// Builds a per-atom displacement-magnitude column from the resident pair.
    ///
    /// Each value is `|end − start|` for the atom over the two frames the scene
    /// already holds — the frame-to-frame motion, the same quantity that drives
    /// the porcupine arrows, reduced to a scalar. It is returned as an immutable
    /// [`AtomProperty`] with [`AtomPropertyMeaning::Flexibility`] so a caller can
    /// colour rigid regions cool and mobile loops hot ("colour by fluctuation")
    /// through the existing property colour path, without re-deriving from the
    /// frame arrays it handed the scene. The column is a snapshot of the current
    /// pair; add it once, then refresh with
    /// [`Scene::replace_atom_property`] as the resident frames advance.
    ///
    /// # Errors
    ///
    /// Returns [`CoreError::StaleHandle`] for an absent structure,
    /// [`CoreError::InvalidTrajectory`] when the structure has no active segment,
    /// and propagates [`AtomProperty::new`] validation.
    pub fn trajectory_displacement_property(
        &self,
        handle: StructureHandle,
        name: impl Into<Arc<str>>,
        semantics: ScalarFieldSemantics,
    ) -> Result<AtomProperty, CoreError> {
        let placed = self.structure(handle).ok_or(CoreError::StaleHandle)?;
        let segment = placed.trajectory().ok_or(CoreError::InvalidTrajectory {
            reason: "structure has no active trajectory segment",
        })?;
        let values: Arc<[f32]> = segment
            .start()
            .positions()
            .iter()
            .zip(segment.end().positions())
            .map(|(from, to)| (Vec3::from_array(*to) - Vec3::from_array(*from)).length())
            .collect();
        AtomProperty::new(
            handle,
            name,
            values,
            AtomPropertyMeaning::Flexibility,
            semantics,
        )
    }

    /// Sets the model-space length past which a stretched bond is hidden.
    ///
    /// A bond in this engine is drawn between two atom indices, so when a
    /// trajectory moves the atoms apart the cylinder stretches indefinitely.
    /// A positive length lets the GPU cull pass drop a bond whose current
    /// endpoints exceed it — the correct behaviour when a covalent bond
    /// dissociates during a reactive or unbinding trajectory. Zero (the
    /// default) disables breaking, so static structures are unaffected. A
    /// generous covalent cutoff (every real covalent bond is under ~2.1 Å)
    /// breaks dissociating bonds without touching intact ones.
    ///
    /// # Errors
    ///
    /// Returns [`CoreError::StaleHandle`] for an absent structure or
    /// [`CoreError::InvalidTrajectory`] for a non-finite or negative length.
    pub fn set_bond_break_length(
        &mut self,
        handle: StructureHandle,
        length: f32,
    ) -> Result<(), CoreError> {
        self.structure_mut(handle)
            .ok_or(CoreError::StaleHandle)?
            .set_bond_break_length(length)
    }

    /// Removes the active interval and restores parsed coordinates.
    ///
    /// # Errors
    ///
    /// Returns [`CoreError::StaleHandle`] after structure removal.
    pub fn clear_trajectory(&mut self, handle: StructureHandle) -> Result<bool, CoreError> {
        Ok(self
            .structure_mut(handle)
            .ok_or(CoreError::StaleHandle)?
            .clear_trajectory())
    }
}
