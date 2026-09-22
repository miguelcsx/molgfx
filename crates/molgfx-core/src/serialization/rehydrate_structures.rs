//! Reconstruction of shared structure assets and placement-local overrides.

use super::super::types::StructureDescription;
use super::{insert, invalid, raw};
use crate::{
    Column, CoreError, DatasetId, PlacedStructure, Scene, StructureAsset, StructureHandle,
};

pub(super) fn rehydrate(
    scene: &mut Scene,
    descriptions: &[StructureDescription],
    sources: &[molframe::Structure],
) -> Result<Vec<StructureHandle>, CoreError> {
    let mut used = vec![false; sources.len()];
    let mut assets = Vec::<StructureAsset>::new();
    let mut handles = Vec::with_capacity(descriptions.len());
    for description in descriptions {
        let dataset = DatasetId::new(description.dataset_id);
        let placed = if dataset == DatasetId::LEGACY {
            find_new_asset(description, sources, &mut used, None)?.1
        } else if let Some(asset) = assets.iter().find(|asset| asset.dataset_id() == dataset) {
            placement(description, asset)?.ok_or_else(|| {
                super::invalid_value("one dataset id resolves to different structure fingerprints")
            })?
        } else {
            let (asset, placed) = find_new_asset(description, sources, &mut used, Some(dataset))?;
            assets.push(asset);
            placed
        };
        let handle = insert_placement(scene, description, placed)?;
        handles.push(handle);
    }
    Ok(handles)
}

fn find_new_asset(
    description: &StructureDescription,
    sources: &[molframe::Structure],
    used: &mut [bool],
    dataset: Option<DatasetId>,
) -> Result<(StructureAsset, PlacedStructure), CoreError> {
    for (index, source) in sources.iter().enumerate() {
        if used[index] {
            continue;
        }
        let id = match dataset {
            Some(dataset) => dataset,
            None => DatasetId::LEGACY,
        };
        let Ok(asset) = StructureAsset::new(id, source) else {
            continue;
        };
        let Some(placed) = placement(description, &asset)? else {
            continue;
        };
        used[index] = true;
        return Ok((asset, placed));
    }
    invalid("no supplied structure matches a manifest fingerprint")
}

fn placement(
    description: &StructureDescription,
    asset: &StructureAsset,
) -> Result<Option<PlacedStructure>, CoreError> {
    let mut placed = PlacedStructure::from_asset(asset);
    if !structure_matches(description, &placed) {
        return Ok(None);
    }
    let transform = molgfx_math::Mat4::from_cols_array(&description.model_to_world);
    if !transform.is_finite() {
        return invalid("structure placement contains a non-finite value");
    }
    placed.model_to_world = transform;
    placed.secondary_structure = Column::new(parse_secondary(
        &description.secondary_structure,
        placed.hierarchy.residue_count(),
    )?);
    Ok(Some(placed))
}

fn insert_placement(
    scene: &mut Scene,
    description: &StructureDescription,
    placed: PlacedStructure,
) -> Result<StructureHandle, CoreError> {
    let raw = raw(description.row, description.generation);
    insert(
        scene.structures.insert_at(raw, placed),
        "structure identity collision",
    )?;
    Ok(StructureHandle(raw))
}

fn structure_matches(description: &StructureDescription, placed: &PlacedStructure) -> bool {
    let metadata_matches = placed.source.molframe().is_some_and(|structure| {
        let entry = structure.metadata();
        entry.id.as_deref() == description.source_id.as_deref()
            && entry.title.as_deref() == description.title.as_deref()
            && entry.method.as_deref() == description.method.as_deref()
            && same_option_bits(entry.resolution, description.resolution)
    });
    metadata_matches
        && placed.atoms.len() == description.atom_count
        && super::super::coordinate_hash::coordinate_hash(placed) == description.coordinate_hash
}

fn parse_secondary(
    values: &[String],
    expected: usize,
) -> Result<Vec<crate::SecondaryStructure>, CoreError> {
    if values.len() != expected {
        return invalid("secondary-structure column length differs from the source");
    }
    values
        .iter()
        .map(|value| match value.as_str() {
            "unknown" => Ok(crate::SecondaryStructure::Unknown),
            "coil" => Ok(crate::SecondaryStructure::Coil),
            "helix" => Ok(crate::SecondaryStructure::Helix),
            "strand" => Ok(crate::SecondaryStructure::Strand),
            "turn" => Ok(crate::SecondaryStructure::Turn),
            _ => invalid("unknown secondary-structure label"),
        })
        .collect()
}

fn same_option_bits(left: Option<f32>, right: Option<f32>) -> bool {
    match (left, right) {
        (Some(left), Some(right)) => left.to_bits() == right.to_bits(),
        (None, None) => true,
        _ => false,
    }
}
