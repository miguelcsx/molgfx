//! Binary scientific assets are staged before one revision-preserving resolution.

use super::{WebScene, javascript_error};
use std::sync::Arc;
use wasm_bindgen::prelude::*;

impl WebScene {
    pub(super) fn require_unresolved(&self) -> Result<(), JsError> {
        if self.resolved.is_some() {
            return Err(JsError::new(
                "restore assets before resolving the authored scene",
            ));
        }
        Ok(())
    }
}

#[wasm_bindgen(js_class = Scene)]
impl WebScene {
    /// Restores an authored scalar grid without recording an edit.
    ///
    /// # Errors
    /// Refuses resolved scenes, undeclared grids and duplicate content identities.
    #[wasm_bindgen(js_name = restoreVolume)]
    pub fn restore_volume(&mut self, identity: u64, values: Vec<f32>) -> Result<(), JsError> {
        self.require_unresolved()?;
        let spec = self
            .spec
            .volumes
            .get(&molgfx::VolumeId::new(identity))
            .ok_or_else(|| JsError::new("unknown volume identity"))?;
        let hash = spec.source.content_hash.clone();
        if self.volume_bindings.contains_key(&hash) {
            return Err(JsError::new("volume is already restored"));
        }
        let binding =
            molgfx::VolumeBinding::new(spec.source.clone(), spec.dimensions, Arc::from(values))
                .affine(spec.voxel_to_world);
        let _ = self.volume_bindings.insert(hash, binding);
        Ok(())
    }

    /// Restores classified donor, hydrogen and acceptor addresses without recording an edit.
    ///
    /// # Errors
    /// Refuses malformed triples, unknown descriptors, duplicate assets or resolved scenes.
    #[wasm_bindgen(js_name = restoreHydrogenBonds)]
    pub fn restore_hydrogen_bonds(
        &mut self,
        identity: u64,
        endpoints: &[u32],
        candidate_distance: Option<f32>,
        pose: Option<String>,
    ) -> Result<(), JsError> {
        self.require_unresolved()?;
        if !endpoints.len().is_multiple_of(3) {
            return Err(JsError::new("hydrogen-bond addresses require triples"));
        }
        let Some(molgfx::InteractionSpec::HydrogenBonds(spec)) = self
            .spec
            .interactions
            .get(&molgfx::InteractionId::new(identity))
        else {
            return Err(JsError::new("identity is not a hydrogen-bond presentation"));
        };
        let hash = spec.source.content_hash.clone();
        if self.hydrogen_bond_bindings.contains_key(&hash) {
            return Err(JsError::new("hydrogen bonds are already restored"));
        }
        let rows = Arc::from(endpoints.as_chunks::<3>().0);
        let binding = match candidate_distance {
            Some(distance) => {
                molgfx::HydrogenBondBinding::candidates(spec.source.clone(), rows, distance)
            }
            None => molgfx::HydrogenBondBinding::new(spec.source.clone(), rows),
        };
        let molecule = self
            .structures
            .get(&spec.structure)
            .ok_or_else(|| JsError::new("bind the hydrogen-bond molecular structure first"))?;
        let interval: Option<(u64, u64, f32)> = match pose {
            Some(metadata) => serde_json::from_str(&metadata).map_err(javascript_error)?,
            None => None,
        };
        let pose = match interval {
            Some((start, end, seconds)) => self
                .resident_binding(
                    molgfx::schema::DataSource::new("hydrogen-bond-pose"),
                    start,
                    end,
                    seconds,
                )?
                .capture_pose(molecule)
                .map_err(javascript_error)?,
            None => molgfx::source::MolecularPose::new(molecule.clone(), None)
                .map_err(javascript_error)?,
        };
        let binding = binding.at_pose(pose);
        let _ = self.hydrogen_bond_bindings.insert(hash, binding);
        Ok(())
    }

    /// Restores provider metadata and little-endian scientific geometry columns.
    /// Molecular atom coordinates remain in the bound source.
    ///
    /// # Errors
    /// Refuses incomplete geometry, invalid ownership or contradictory scientific controls.
    #[wasm_bindgen(js_name = restoreMolecularSurface)]
    pub fn restore_molecular_surface(
        &mut self,
        identity: u64,
        metadata: &str,
        vertices: &[f32],
        normals: &[f32],
        faces: &[u32],
        atoms: Vec<u32>,
    ) -> Result<(), JsError> {
        self.require_unresolved()?;
        if !vertices.len().is_multiple_of(3)
            || !normals.len().is_multiple_of(3)
            || !faces.len().is_multiple_of(3)
        {
            return Err(JsError::new("molecular surface columns require triples"));
        }
        let identity = molgfx::RepresentationId::new(identity);
        if self.molecular_surface_bindings.contains_key(&identity) {
            return Err(JsError::new("molecular surface is already restored"));
        }
        let recipe = self
            .spec
            .representations
            .get(&identity)
            .ok_or_else(|| JsError::new("unknown representation identity"))?;
        let (source, _, _) = recipe
            .molecular_surface_recipe()
            .ok_or_else(|| JsError::new("representation is not an analytical surface"))?;
        let structure = recipe
            .structure_id()
            .ok_or_else(|| JsError::new("surface has no structure"))?;
        let molecule = self
            .structures
            .get(&structure)
            .ok_or_else(|| JsError::new("bind the surface's molecular structure first"))?;
        let SurfaceMetadata(method, controls, discarded_degenerate_faces, interval) =
            serde_json::from_str(metadata).map_err(javascript_error)?;
        let mesh = molgfx::schema::MolecularSurfaceDescription {
            vertices: vertices.as_chunks::<3>().0.to_vec(),
            normals: normals.as_chunks::<3>().0.to_vec(),
            faces: faces.as_chunks::<3>().0.to_vec(),
            atom_rows: atoms,
            discarded_degenerate_faces,
        };
        let binding = molgfx::MolecularSurfaceBinding::new(
            source.clone(),
            structure,
            molecule,
            method,
            controls,
            mesh.into_geometry().map_err(javascript_error)?,
        )
        .map_err(javascript_error)?;
        let binding = match interval {
            Some((start, end, seconds)) => {
                let pose = self
                    .resident_binding(
                        molgfx::schema::DataSource::new("surface-pose"),
                        start,
                        end,
                        seconds,
                    )?
                    .capture_pose(molecule)
                    .map_err(javascript_error)?;
                binding.at_pose(&pose).map_err(javascript_error)?
            }
            None => binding,
        };
        let _ = self
            .molecular_surface_bindings
            .insert(identity, (recipe.clone(), binding));
        Ok(())
    }
}

#[derive(serde::Deserialize)]
struct SurfaceMetadata(
    molgfx::MolecularSurfaceMethod,
    molgfx::MolecularSurfaceControls,
    u64,
    #[serde(default)] Option<(u64, u64, f32)>,
);
