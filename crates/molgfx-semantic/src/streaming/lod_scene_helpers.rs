use super::{DetailBinding, LodBinding};
use crate::LodLevel;
use crate::streaming::plan::{LodClusterKey, LodIndex};
use molgfx_core::{
    CoreError, Particle, ParticleShape, Primitive, PrimitiveHandle, RepresentationHandle, Scene,
    StructureHandle,
};
use molgfx_math::{Quat, Rgba8, Vec3};

pub(super) fn binding(
    scene: &mut Scene,
    index: &LodIndex,
    key: LodClusterKey,
    reuse: Option<PrimitiveHandle>,
) -> Result<LodBinding, CoreError> {
    let cluster = index.cluster(key).ok_or(CoreError::InvalidPrimitive {
        reason: "LOD frame refers to a missing cluster",
    })?;
    let placed = scene
        .structure(key.structure)
        .ok_or(CoreError::StaleHandle)?;
    let center = placed
        .model_to_world
        .inverse()
        .transform_point3(cluster.center);
    let scale = [
        placed.model_to_world.transform_vector3(Vec3::X).length(),
        placed.model_to_world.transform_vector3(Vec3::Y).length(),
        placed.model_to_world.transform_vector3(Vec3::Z).length(),
    ]
    .into_iter()
    .fold(f32::INFINITY, f32::min)
    .max(1.0e-3);
    let particle = Particle::new(
        key.structure,
        center,
        Quat::IDENTITY,
        Vec3::splat(cluster.radius / scale * 2.0),
        ParticleShape::Sphere,
        color_for(key.level),
        0.82,
    )?;
    let primitive = match reuse {
        Some(handle) => {
            let slot = scene.primitive_mut(handle).ok_or(CoreError::StaleHandle)?;
            *slot = Primitive::particle(particle);
            handle
        }
        None => scene
            .add_primitives(&[Primitive::particle(particle)])?
            .ok_or(CoreError::InvalidPrimitive {
                reason: "non-empty LOD batch produced no handle",
            })?,
    };
    Ok(LodBinding {
        key,
        structure: key.structure,
        primitive,
        base_opacity: 0.82,
    })
}

pub(super) fn set_binding_alpha(scene: &mut Scene, binding: LodBinding, alpha: f32) {
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

pub(super) fn set_detail_alpha(scene: &mut Scene, binding: DetailBinding, alpha: f32) {
    let target_opacity = binding.base_opacity * clamp_unit(alpha);
    let target_visible = binding.base_visible && target_opacity > 1.0e-4;
    let Some(value) = scene.representation(binding.representation) else {
        return;
    };
    if value.visible == target_visible && (value.material.opacity - target_opacity).abs() <= 1.0e-6
    {
        return;
    }
    if let Some(value) = scene.representation_mut(binding.representation) {
        value.visible = target_visible;
        value.material.opacity = target_opacity;
    }
}

pub(super) fn bind_native(
    bindings: &mut Vec<DetailBinding>,
    scene: &Scene,
    structure: StructureHandle,
    representation: RepresentationHandle,
) -> Result<(), CoreError> {
    if scene.structure(structure).is_none() {
        return Err(CoreError::StaleHandle);
    }
    let value = scene
        .representation(representation)
        .ok_or(CoreError::StaleHandle)?;
    match bindings.binary_search_by_key(&representation, |binding| binding.representation) {
        Ok(row) if bindings[row].structure != structure => {
            return Err(CoreError::InvalidPrimitive {
                reason: "LOD representation is already bound to another structure",
            });
        }
        Ok(_) => {}
        Err(row) => bindings.insert(
            row,
            DetailBinding {
                structure,
                representation,
                base_opacity: value.material.opacity,
                base_visible: value.visible,
            },
        ),
    }
    Ok(())
}

pub(super) fn unbind_native(
    bindings: &mut Vec<DetailBinding>,
    scene: &mut Scene,
    representation: RepresentationHandle,
) {
    let Ok(row) = bindings.binary_search_by_key(&representation, |binding| binding.representation)
    else {
        return;
    };
    let binding = bindings.remove(row);
    set_detail_alpha(scene, binding, 1.0);
}

pub(super) fn has_native_binding(bindings: &[DetailBinding], structure: StructureHandle) -> bool {
    bindings
        .iter()
        .any(|binding| binding.structure == structure)
}

pub(super) fn clamp_unit(value: f32) -> f32 {
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
