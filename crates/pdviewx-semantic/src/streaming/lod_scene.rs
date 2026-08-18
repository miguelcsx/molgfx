//! Scene-facing application of the semantic LOD decision.
//!
//! The core scene stays independent of this policy crate. This adapter lowers
//! visible coarse clusters to persistent particle records, reuses their
//! handles across frames, and leaves the caller's native atom representations
//! untouched when atom detail is chosen.

use super::plan::{LodFrame, LodIndex};
use crate::LodLevel;
use pdviewx_core::{
    CoreError, Particle, ParticleShape, Primitive, PrimitiveHandle, Scene, StructureHandle,
};
use pdviewx_math::{Quat, Rgba8, Vec3};

/// Persistent coarse analytic particles owned by a semantic LOD controller.
#[derive(Clone, Debug, Default)]
pub struct LodScene {
    bindings: Vec<LodBinding>,
}

#[derive(Clone, Copy, Debug)]
struct LodBinding {
    key: super::plan::LodClusterKey,
    structure: StructureHandle,
    primitive: PrimitiveHandle,
    base_opacity: f32,
}

impl LodScene {
    /// Applies one selected frame while reusing the existing scene handles.
    ///
    /// Each visible residue, secondary-structure or domain cluster becomes one
    /// analytic sphere at the cluster centroid and radius. All records share
    /// the core scientific table and its single indirect draw, so the coarse
    /// path does not add one CPU draw call per biological cluster. The caller's
    /// atom-level representations are enabled or disabled by the caller from
    /// [`LodFrame::atom_structures`].
    ///
    /// # Errors
    ///
    /// Returns [`CoreError::StaleHandle`] for a removed structure or
    /// [`CoreError::InvalidPrimitive`] for an inconsistent frame.
    pub fn apply(
        &mut self,
        scene: &mut Scene,
        index: &LodIndex,
        frame: &LodFrame,
    ) -> Result<(), CoreError> {
        self.remove_stale(scene);
        for &key in frame.visible() {
            if key.level == LodLevel::Atom || self.bindings.iter().any(|binding| binding.key == key)
            {
                continue;
            }
            self.create_binding(scene, index, key)?;
        }
        for binding in &self.bindings {
            let selected = frame.visible().binary_search(&binding.key).is_ok();
            set_binding_alpha(scene, *binding, f32::from(selected));
        }
        Ok(())
    }

    /// Cross-fades persistent coarse records between two selected frames.
    ///
    /// The transition is deterministic and only changes the shared analytic
    /// particle table. Atom-level representations remain caller-owned because
    /// their native atom material may be controlled independently of semantic
    /// LOD; callers should fade that representation with the same `weight`.
    ///
    /// # Errors
    ///
    /// Returns [`CoreError::StaleHandle`] for a removed structure or
    /// [`CoreError::InvalidPrimitive`] for an inconsistent frame.
    pub fn apply_transition(
        &mut self,
        scene: &mut Scene,
        index: &LodIndex,
        from: &LodFrame,
        to: &LodFrame,
        weight: f32,
    ) -> Result<(), CoreError> {
        self.remove_stale(scene);
        let weight = clamp_unit(weight);
        let mut keys = Vec::with_capacity(from.visible().len() + to.visible().len());
        keys.extend(
            from.visible()
                .iter()
                .chain(to.visible())
                .copied()
                .filter(|key| key.level != LodLevel::Atom),
        );
        keys.sort_unstable();
        keys.dedup();
        for key in keys {
            if !self.bindings.iter().any(|binding| binding.key == key) {
                self.create_binding(scene, index, key)?;
            }
        }
        for binding in &self.bindings {
            let was_visible = from.visible().binary_search(&binding.key).is_ok();
            let is_visible = to.visible().binary_search(&binding.key).is_ok();
            let alpha = match (was_visible, is_visible) {
                (true, true) => 1.0,
                (true, false) => 1.0 - weight,
                (false, true) => weight,
                (false, false) => 0.0,
            };
            set_binding_alpha(scene, *binding, alpha);
        }
        Ok(())
    }

    /// Number of persistent coarse analytic records currently owned.
    #[must_use]
    pub const fn primitive_count(&self) -> usize {
        self.bindings.len()
    }

    fn create_binding(
        &mut self,
        scene: &mut Scene,
        index: &LodIndex,
        key: super::plan::LodClusterKey,
    ) -> Result<(), CoreError> {
        let cluster = index.cluster(key).ok_or(CoreError::InvalidPrimitive {
            reason: "LOD frame refers to a missing cluster",
        })?;
        let placed = scene
            .structure(key.structure)
            .ok_or(CoreError::StaleHandle)?;
        let inverse = placed.model_to_world.inverse();
        let center = inverse.transform_point3(cluster.center);
        let scale = [
            placed.model_to_world.transform_vector3(Vec3::X).length(),
            placed.model_to_world.transform_vector3(Vec3::Y).length(),
            placed.model_to_world.transform_vector3(Vec3::Z).length(),
        ]
        .into_iter()
        .fold(f32::INFINITY, f32::min)
        .max(1.0e-3);
        let radius = cluster.radius / scale;
        let particle = Particle::new(
            key.structure,
            center,
            Quat::IDENTITY,
            Vec3::splat(radius * 2.0),
            ParticleShape::Sphere,
            color_for(key.level),
            0.82,
        )?;
        let primitive = scene.add_particle(particle)?;
        self.bindings.push(LodBinding {
            key,
            structure: key.structure,
            primitive,
            base_opacity: 0.82,
        });
        Ok(())
    }

    fn remove_stale(&mut self, scene: &mut Scene) {
        let old = std::mem::take(&mut self.bindings);
        for binding in old {
            if scene.structure(binding.structure).is_some()
                && scene.primitive(binding.primitive).is_some()
            {
                self.bindings.push(binding);
            } else {
                scene.remove_primitive(binding.primitive);
            }
        }
    }
}

fn set_binding_alpha(scene: &mut Scene, binding: LodBinding, alpha: f32) {
    let target_opacity = binding.base_opacity * clamp_unit(alpha);
    let target_visible = target_opacity > 1.0e-4;
    let Some(Primitive::Particle(value)) = scene.primitive(binding.primitive).copied() else {
        return;
    };
    if value.visible == target_visible && (value.opacity - target_opacity).abs() <= 1.0e-6 {
        return;
    }
    if let Some(Primitive::Particle(value)) = scene.primitive_mut(binding.primitive) {
        value.visible = target_visible;
        value.opacity = target_opacity;
    }
}

fn clamp_unit(value: f32) -> f32 {
    if value.is_finite() {
        value.clamp(0.0, 1.0)
    } else if value.is_sign_positive() {
        1.0
    } else {
        0.0
    }
}

fn color_for(level: LodLevel) -> Rgba8 {
    match level {
        LodLevel::Atom => Rgba8::opaque(220, 230, 240),
        LodLevel::Residue => Rgba8::opaque(95, 170, 235),
        LodLevel::SecondaryStructure => Rgba8::opaque(120, 205, 170),
        LodLevel::Domain => Rgba8::opaque(230, 175, 90),
    }
}
