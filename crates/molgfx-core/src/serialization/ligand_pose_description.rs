//! Stable scene-manifest payloads for compact ligand pose batches.

use super::rehydrate::{insert, invalid, raw, resolve_structure};
use super::types::ObjectIdentity;
use crate::{LicoriceTemplate, LigandPose, LigandPoseBatch, Scene, StructureHandle};
use molgfx_math::{Quat, Rgba8, Vec3};
use serde::{Deserialize, Serialize};

/// One rigid ligand pose in a scene manifest.
#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
pub struct LigandPoseDescription {
    /// Destination of the template origin.
    pub translation: [f32; 3],
    /// Unit rotation applied before translation.
    pub orientation: [f32; 4],
    /// Candidate display colour.
    pub color: [u8; 4],
    /// Candidate opacity.
    pub opacity: f32,
}

/// One compact reusable topology and its rigid occurrences.
#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
pub struct LigandPoseBatchDescription {
    /// Stable batch row.
    pub row: u32,
    /// Stable batch generation.
    pub generation: u32,
    /// Owning structure identity.
    pub owner: ObjectIdentity,
    /// Local atom centres relative to the template origin.
    pub atoms: Vec<[f32; 3]>,
    /// Zero-based topology edges.
    pub bonds: Vec<[u32; 2]>,
    /// Sphere radius in Angstrom.
    pub atom_radius: f32,
    /// Capsule radius in Angstrom.
    pub bond_radius: f32,
    /// Compact rigid candidate column.
    pub poses: Vec<LigandPoseDescription>,
    /// Whether this batch participates in rendering.
    pub visible: bool,
}

pub(crate) fn records(scene: &Scene) -> Vec<LigandPoseBatchDescription> {
    scene
        .ligand_pose_batches()
        .map(|(handle, batch)| {
            let template = batch.template();
            LigandPoseBatchDescription {
                row: handle.row(),
                generation: handle.generation(),
                owner: identity(batch.owner()),
                atoms: template.atom_centers().iter().map(Vec3::to_array).collect(),
                bonds: template.bond_indices().to_vec(),
                atom_radius: template.atom_radius(),
                bond_radius: template.bond_radius(),
                poses: batch
                    .poses()
                    .iter()
                    .copied()
                    .map(pose_description)
                    .collect(),
                visible: batch.visible(),
            }
        })
        .collect()
}

pub(crate) fn rehydrate(
    scene: &mut Scene,
    descriptions: &[LigandPoseBatchDescription],
    structures: &[StructureHandle],
    declared_count: u64,
) -> Result<(), crate::CoreError> {
    if u64::try_from(descriptions.len()).ok() != Some(declared_count) {
        return invalid("ligand pose batch count differs from its manifest table");
    }
    if descriptions
        .iter()
        .any(|description| description.row > crate::EntityId::MAX_INDEX)
    {
        return invalid("ligand pose batch row exceeds the picking range");
    }
    for description in descriptions {
        if description.poses.is_empty() {
            return invalid("ligand pose batch must contain at least one pose");
        }
        let owner = resolve_structure(structures, description.owner)?;
        let atoms = description
            .atoms
            .iter()
            .copied()
            .map(Vec3::from_array)
            .collect();
        let template = LicoriceTemplate::new(Vec3::ZERO, atoms, &description.bonds)?
            .radii(description.atom_radius, description.bond_radius)?;
        let Some(instance_count) = template
            .instances_per_pose()
            .checked_mul(description.poses.len())
        else {
            return invalid("ligand pose batch is too large");
        };
        if u32::try_from(instance_count).is_err() {
            return invalid("ligand pose batch exceeds the GPU draw range");
        }
        let poses = description
            .poses
            .iter()
            .map(pose)
            .collect::<Result<Vec<_>, _>>()?;
        if poses
            .iter()
            .any(|pose| !pose.transforms_finitely(template.bound_radius()))
        {
            return invalid("ligand pose batch transform overflows model space");
        }
        let mut batch = LigandPoseBatch::new(owner, template, poses);
        batch.set_visible(description.visible);
        insert(
            scene
                .ligand_pose_batches
                .insert_at(raw(description.row, description.generation), batch),
            "ligand pose batch identity collision",
        )?;
    }
    Ok(())
}

fn pose_description(value: LigandPose) -> LigandPoseDescription {
    let color = value.color();
    LigandPoseDescription {
        translation: value.translation().to_array(),
        orientation: value.orientation().to_array(),
        color: [color.r, color.g, color.b, color.a],
        opacity: value.opacity(),
    }
}

fn pose(value: &LigandPoseDescription) -> Result<LigandPose, crate::CoreError> {
    LigandPose::new(
        Vec3::from_array(value.translation),
        Quat::from_array(value.orientation),
        Rgba8::new(
            value.color[0],
            value.color[1],
            value.color[2],
            value.color[3],
        ),
        value.opacity,
    )
}

const fn identity(handle: StructureHandle) -> ObjectIdentity {
    ObjectIdentity {
        row: handle.row(),
        generation: handle.generation(),
    }
}

#[cfg(test)]
#[path = "ligand_pose_description_tests.rs"]
mod tests;
