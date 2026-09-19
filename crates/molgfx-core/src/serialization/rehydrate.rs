//! Cold-source scene reconstruction from a stable manifest.

#[path = "rehydrate_generic.rs"]
mod generic;
#[path = "rehydrate_payload.rs"]
mod payload;
#[path = "rehydrate_render.rs"]
mod render;
#[path = "rehydrate_rows.rs"]
mod rows;
#[path = "rehydrate_structures.rs"]
mod structures;
#[path = "rehydrate_volumes.rs"]
mod volumes;

use super::types::{
    AtomPropertyDescription, MeshDescription, MeshInstanceDescription, ObjectIdentity,
    OverlayDescription, ScalarSemanticsDescription, SelectionDescription, VolumeDescription,
};
use super::{
    GenericSceneDescriptionSources, SceneDescription, SceneDescriptionSources,
    manifest::SCHEMA_VERSION,
};
use crate::handle::{RawHandle, StructureHandle};
use crate::scene::{Scene, StoredAtomProperty, StoredSegmentation, StoredSelection};
use crate::{AtomProperty, AtomPropertyMeaning, AtomSelection, CoreError, ScalarFieldSemantics};
use roaring::RoaringBitmap;

impl Scene {
    /// Rebuilds a scene from a manifest and caller-resupplied source payloads.
    ///
    /// Structures, volumes, segmentations and properties are never embedded in
    /// the JSON description. Their hashes, dimensions and transforms are
    /// checked before the scene is returned, so a mismatch cannot silently
    /// produce a different figure.
    ///
    /// # Errors
    ///
    /// Returns [`CoreError::InvalidSceneDescription`] when the schema, source
    /// fingerprints, handles or scene-owned records do not match.
    pub fn from_description(
        description: &SceneDescription,
        sources: SceneDescriptionSources<'_>,
    ) -> Result<Self, CoreError> {
        Self::from_description_with_generic(
            description,
            sources,
            GenericSceneDescriptionSources::default(),
        )
    }

    /// Rebuilds schema-8 generic row tables from caller-retained immutable
    /// payloads in addition to the structure and volume sources.
    ///
    /// # Errors
    ///
    /// Returns [`CoreError::InvalidSceneDescription`] when counts, hashes,
    /// domains, handles or visual programs disagree.
    pub fn from_description_with_generic(
        description: &SceneDescription,
        sources: SceneDescriptionSources<'_>,
        generic_sources: GenericSceneDescriptionSources<'_>,
    ) -> Result<Self, CoreError> {
        validate_header(description)?;
        if sources.volumes.len()
            != description
                .volumes
                .iter()
                .filter(|volume| volume.occupancy.is_none())
                .count()
            || sources.segmentations.len() != description.segmentations.len()
            || sources.atom_properties.len() != description.atom_properties.len()
            || sources.meshes.len() != description.meshes.len()
            || generic_sources.point_batches.len() != description.point_batches.len()
            || generic_sources.instance_batches.len() != description.instance_batches.len()
            || generic_sources.attributes.len() != description.attributes.len()
            || generic_sources.relation_batches.len() != description.relation_batches.len()
        {
            return invalid("caller sources do not match manifest table counts");
        }
        let mut scene = Scene::new();
        rows::prepare(&mut scene, description)?;
        let structures =
            structures::rehydrate(&mut scene, &description.structures, sources.structures)?;
        volumes::rehydrate(
            &mut scene,
            &description.volumes,
            sources.volumes,
            &structures,
        )?;
        rehydrate_segmentations(
            &mut scene,
            &description.segmentations,
            sources.segmentations,
        )?;
        rehydrate_properties(
            &mut scene,
            &description.atom_properties,
            sources.atom_properties,
            &structures,
        )?;
        rehydrate_selections(&mut scene, &description.selections, &structures)?;
        rehydrate_meshes(&mut scene, &description.meshes, sources.meshes, &structures)?;
        rehydrate_mesh_instances(&mut scene, &description.mesh_instances)?;
        render::rehydrate_representations(&mut scene, &description.representations)?;
        payload::rehydrate_primitives(&mut scene, &description.primitives)?;
        super::ligand_pose_description::rehydrate(
            &mut scene,
            &description.ligand_pose_batches,
            &structures,
            description.tables.ligand_pose_batches,
        )?;
        generic::rehydrate(&mut scene, description, generic_sources)?;
        payload::rehydrate_guides(&mut scene, &description.guides)?;
        payload::rehydrate_interactions(&mut scene, &description.interactions)?;
        payload::rehydrate_labels(
            &mut scene,
            &description.annotations,
            &description.measurements,
        )?;
        rehydrate_overlays(&mut scene, &description.overlays)?;
        scene.validate_description(description)?;
        Ok(scene)
    }
}

fn validate_header(description: &SceneDescription) -> Result<(), CoreError> {
    if description.schema != SCHEMA_VERSION {
        return invalid("unsupported scene schema");
    }
    if description.engine != format!("molgfx-scene-{SCHEMA_VERSION}") {
        return invalid("scene engine format does not match the schema");
    }
    Ok(())
}

fn rehydrate_meshes(
    scene: &mut Scene,
    descriptions: &[MeshDescription],
    sources: &[crate::Mesh],
    structures: &[StructureHandle],
) -> Result<(), CoreError> {
    for (description, source) in descriptions.iter().zip(sources) {
        if crate::serialization::records::mesh_hash(source) != description.content_hash {
            return invalid("supplied mesh does not match its manifest");
        }
        let mut mesh = source.clone();
        mesh.set_owner(resolve_structure(structures, description.owner)?);
        let raw = raw(description.row, description.generation);
        insert(scene.meshes.insert_at(raw, mesh), "mesh identity collision")?;
    }
    Ok(())
}

fn rehydrate_mesh_instances(
    scene: &mut Scene,
    descriptions: &[MeshInstanceDescription],
) -> Result<(), CoreError> {
    for description in descriptions {
        let mesh_raw = resolve_raw(description.mesh);
        resolve_existing(scene.meshes.get(mesh_raw))?;
        let mut value = crate::MeshInstance::new(
            crate::MeshHandle(mesh_raw),
            molgfx_math::Mat4::from_cols_array(&description.transform),
        )?;
        value.set_visible(description.visible);
        insert(
            scene
                .mesh_instances
                .insert_at(raw(description.row, description.generation), value),
            "mesh instance identity collision",
        )?;
    }
    Ok(())
}

fn rehydrate_overlays(
    scene: &mut Scene,
    descriptions: &[OverlayDescription],
) -> Result<(), CoreError> {
    for description in descriptions {
        let anchor = crate::OverlayAnchor::new(description.normalized, description.pixels)?;
        let color =
            |value: [u8; 4]| molgfx_math::Rgba8::new(value[0], value[1], value[2], value[3]);
        let content = match description.kind.as_str() {
            "text" => crate::OverlayContent::Text {
                text: description.text.clone(),
                color: color(description.colors[0]),
                size_pixels: description.values[0],
            },
            "color-legend" => crate::OverlayContent::ColorLegend {
                title: description.text.clone(),
                range: [description.values[0], description.values[1]],
                colors: [color(description.colors[0]), color(description.colors[1])],
                size_pixels: [description.values[2], description.values[3]],
            },
            "scale-bar" => crate::OverlayContent::ScaleBar {
                length_angstrom: description.values[0],
                color: color(description.colors[0]),
                width_pixels: description.values[1],
            },
            "coordinate-tripod" => crate::OverlayContent::CoordinateTripod {
                size_pixels: description.values[0],
                width_pixels: description.values[1],
            },
            _ => return invalid("unknown screen overlay kind"),
        };
        let mut value = crate::ScreenOverlay::new(content, anchor)?;
        value.set_order(description.order);
        value.set_visible(description.visible);
        insert(
            scene
                .overlays
                .insert_at(raw(description.row, description.generation), value),
            "overlay identity collision",
        )?;
    }
    Ok(())
}

fn rehydrate_segmentations(
    scene: &mut Scene,
    descriptions: &[VolumeDescription],
    sources: &[crate::SegmentedVolume],
) -> Result<(), CoreError> {
    for (description, value) in descriptions.iter().zip(sources) {
        if value.dimensions() != description.dimensions
            || !same_array_bits(
                value.voxel_to_world().to_cols_array(),
                description.voxel_to_world,
            )
            || label_hash(value.labels()) != description.content_hash
        {
            return invalid("supplied segmentation does not match its manifest");
        }
        let raw = raw(description.row, description.generation);
        insert(
            scene.segmentations.insert_at(
                raw,
                StoredSegmentation {
                    value: value.clone(),
                    revision: 0,
                },
            ),
            "segmentation identity collision",
        )?;
    }
    Ok(())
}

fn rehydrate_properties(
    scene: &mut Scene,
    descriptions: &[AtomPropertyDescription],
    sources: &[AtomProperty],
    structures: &[StructureHandle],
) -> Result<(), CoreError> {
    for (description, source) in descriptions.iter().zip(sources) {
        let owner = resolve_structure(structures, description.owner)?;
        if source.name() != description.name
            || !length_matches(source.values().len(), description.length)
            || !same_array_bits(source.finite_domain(), description.finite_domain)
            || value_hash(source.values()) != description.content_hash
            || property_meaning(source.meaning()) != description.meaning
            || scalar_semantics(source.semantics()) != description.semantics
        {
            return invalid("supplied atom property does not match its manifest");
        }
        let value = AtomProperty::new(
            owner,
            std::sync::Arc::from(source.name()),
            std::sync::Arc::from(source.values()),
            source.meaning(),
            source.semantics().clone(),
        )?;
        let raw = raw(description.row, description.generation);
        insert(
            scene
                .properties
                .insert_at(raw, StoredAtomProperty { value, revision: 0 }),
            "atom property identity collision",
        )?;
    }
    Ok(())
}

fn rehydrate_selections(
    scene: &mut Scene,
    descriptions: &[SelectionDescription],
    structures: &[StructureHandle],
) -> Result<(), CoreError> {
    for description in descriptions {
        let mut scoped = Vec::with_capacity(description.masks.len());
        for mask in &description.masks {
            let structure = structures
                .iter()
                .find(|handle| handle.row() == mask.structure_row)
                .copied()
                .ok_or_else(|| invalid_value("selection references an absent structure"))?;
            let length = scene
                .structure(structure)
                .map(|placed| placed.atoms.len())
                .ok_or(CoreError::StaleHandle)?;
            if mask.atoms.iter().any(|&atom| atom >= length) {
                return invalid("selection contains an out-of-range atom row");
            }
            let mut bitmap = RoaringBitmap::new();
            for &atom in &mask.atoms {
                bitmap.insert(atom);
            }
            scoped.push((structure, AtomSelection::Roaring(bitmap)));
        }
        scoped.sort_unstable_by_key(|(handle, _)| *handle);
        let raw = raw(description.row, description.generation);
        insert(
            scene.selections.insert_at(
                raw,
                StoredSelection {
                    global: None,
                    scoped,
                },
            ),
            "selection identity collision",
        )?;
    }
    Ok(())
}

pub(crate) fn resolve_structure(
    structures: &[StructureHandle],
    identity: ObjectIdentity,
) -> Result<StructureHandle, CoreError> {
    structures
        .iter()
        .find(|handle| handle.row() == identity.row && handle.generation() == identity.generation)
        .copied()
        .ok_or_else(|| invalid_value("manifest references an absent structure"))
}

pub(crate) fn resolve_raw(identity: ObjectIdentity) -> RawHandle {
    RawHandle::from_parts(identity.row, identity.generation)
}

pub(crate) fn resolve_existing<T>(value: Option<&T>) -> Result<&T, CoreError> {
    value.ok_or_else(|| invalid_value("manifest references an absent object"))
}

pub(crate) fn invalid<T>(summary: &'static str) -> Result<T, CoreError> {
    Err(CoreError::InvalidSceneDescription {
        summary: summary.to_owned(),
    })
}

pub(crate) fn invalid_value(summary: &'static str) -> CoreError {
    CoreError::InvalidSceneDescription {
        summary: summary.to_owned(),
    }
}

pub(crate) fn raw(row: u32, generation: u32) -> RawHandle {
    RawHandle::from_parts(row, generation)
}

pub(crate) fn insert(result: Option<()>, summary: &'static str) -> Result<(), CoreError> {
    if result.is_none() {
        return invalid(summary);
    }
    Ok(())
}

fn length_matches(length: usize, expected: u64) -> bool {
    u64::try_from(length).ok() == Some(expected)
}

pub(super) fn same_array_bits<const N: usize>(left: [f32; N], right: [f32; N]) -> bool {
    left.into_iter()
        .zip(right)
        .all(|(left, right)| left.to_bits() == right.to_bits())
}

pub(super) fn value_hash(values: &[f32]) -> u64 {
    super::records::value_hash(values)
}

fn label_hash(values: &[u32]) -> u64 {
    super::records::label_hash(values)
}

fn property_meaning(value: AtomPropertyMeaning) -> &'static str {
    super::records::property_meaning(value)
}

fn scalar_semantics(value: &ScalarFieldSemantics) -> ScalarSemanticsDescription {
    super::records::scalar_semantics(value)
}
