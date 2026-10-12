//! Explicit analytical surface methods and immutable provider bindings.
use crate::{DataSource, Error, StructureId};
use serde::{Deserialize, Serialize};

pub use molgfx_core::{MolecularSurfaceGeometry, MolecularSurfaceSource};

/// Scientific molecular-surface provider named by a portable representation.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MolecularSurfaceMethod {
    /// Varshney's analytical rolling-probe surface.
    Surf,
    /// Sanner's analytical rolling-probe triangulation.
    Msms,
}

/// Scientific controls that must agree with the geometry's actual computation.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct MolecularSurfaceControls {
    /// Rolling-probe radius, angstroms.
    pub probe_radius: f32,
    /// MSMS vertices per square angstrom; unused by Surf.
    pub density: f32,
    /// Include disconnected components and internal cavities in MSMS.
    pub all_components: bool,
}

impl Default for MolecularSurfaceControls {
    fn default() -> Self {
        Self {
            probe_radius: 1.4,
            density: 3.0,
            all_components: true,
        }
    }
}

impl MolecularSurfaceControls {
    pub(crate) fn validate(self) -> Result<(), Error> {
        if !self.probe_radius.is_finite()
            || self.probe_radius <= 0.0
            || !self.density.is_finite()
            || self.density <= 0.0
        {
            return Err(Error::InvalidSpec(
                "analytical surface controls must be finite and positive".to_owned(),
            ));
        }
        Ok(())
    }
}

/// Runtime surface asset tied to the original molecule and coordinate revision.
#[derive(Clone, Debug)]
pub struct MolecularSurfaceBinding {
    pub(crate) source: DataSource,
    pub(crate) structure: StructureId,
    pub(crate) molecular_key: [u64; 2],
    pub(crate) method: MolecularSurfaceMethod,
    pub(crate) controls: MolecularSurfaceControls,
    pub(crate) geometry: MolecularSurfaceGeometry,
}

impl MolecularSurfaceBinding {
    /// Portable content identity of this immutable scientific asset.
    #[must_use]
    pub const fn source(&self) -> &DataSource {
        &self.source
    }
    /// Actual scientific method of this immutable asset.
    #[must_use]
    pub const fn method(&self) -> MolecularSurfaceMethod {
        self.method
    }

    /// Actual numerical controls used by the scientific provider.
    #[must_use]
    pub const fn controls(&self) -> MolecularSurfaceControls {
        self.controls
    }

    /// Borrows the shared validated geometry, including original atom ownership.
    #[must_use]
    pub const fn geometry(&self) -> &MolecularSurfaceGeometry {
        &self.geometry
    }
    /// Records the exact sampled coordinates used for a scientific calculation.
    ///
    /// # Errors
    /// Refuses a captured pose belonging to a different molecular input.
    pub fn at_pose(mut self, pose: &crate::source::MolecularPose) -> Result<Self, Error> {
        self.validate_molecule(pose.source())?;
        self.geometry = self.geometry.for_pose(pose);
        Ok(self)
    }
    /// Binds complete provider output without copying it or inferring atom owners.
    /// The controls must describe the computation that produced `geometry`.
    ///
    /// # Errors
    /// Rejects empty identities, invalid controls or atom rows absent in the molecule.
    pub fn new(
        source: DataSource,
        structure: StructureId,
        molecule: &crate::source::MolecularSource,
        method: MolecularSurfaceMethod,
        controls: MolecularSurfaceControls,
        geometry: MolecularSurfaceGeometry,
    ) -> Result<Self, Error> {
        if source.content_hash.trim().is_empty() {
            return Err(Error::InvalidSpec(
                "analytical surface binding requires a content identity".to_owned(),
            ));
        }
        controls.validate()?;
        geometry.validate_atom_count(molecule.coordinates().len())?;
        Ok(Self {
            source,
            structure,
            molecular_key: [molecule.identity(), molecule.coordinate_revision()],
            method,
            controls,
            geometry: geometry.for_source(molecule),
        })
    }

    /// Retains a native scientific result and takes controls from the actual invocation.
    ///
    /// # Errors
    /// Rejects invalid geometry, a missing content identity or absent atom owners.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn from_native(
        source: DataSource,
        structure: StructureId,
        molecule: &crate::source::MolecularSource,
        output: std::sync::Arc<molframe::surface::native_solver::NativeSurfaceMesh>,
    ) -> Result<Self, Error> {
        let method = match output.provider() {
            molframe::surface::native_solver::NativeSurfaceProvider::Surf => {
                MolecularSurfaceMethod::Surf
            }
            molframe::surface::native_solver::NativeSurfaceProvider::Msms => {
                MolecularSurfaceMethod::Msms
            }
        };
        let options = output.options();
        let controls = MolecularSurfaceControls {
            probe_radius: options.probe_radius,
            density: options.density,
            all_components: options.all_components,
        };
        Self::new(
            source,
            structure,
            molecule,
            method,
            controls,
            MolecularSurfaceGeometry::new(output)?,
        )
    }

    pub(crate) fn same_computation(&self, other: &Self) -> bool {
        if self.structure != other.structure
            || self.method != other.method
            || self.controls != other.controls
        {
            return false;
        }
        if self.geometry == other.geometry {
            return true;
        }
        self.geometry.same_pose(&other.geometry) && self.geometry.same_content(&other.geometry)
    }

    pub(crate) fn validate_molecule(
        &self,
        molecule: &crate::source::MolecularSource,
    ) -> Result<(), Error> {
        if self.molecular_key != [molecule.identity(), molecule.coordinate_revision()] {
            return Err(Error::InvalidSpec("analytical surface geometry belongs to a different molecule or coordinate revision".to_owned()));
        }
        self.geometry
            .validate_atom_count(molecule.coordinates().len())?;
        Ok(())
    }

    pub(crate) fn matches(
        &self,
        spec: &super::form::RepresentationSpec,
        molecule: &crate::source::MolecularSource,
    ) -> Result<(), Error> {
        let super::form::RepresentationFormSpec::MolecularSurface {
            method, controls, ..
        } = &spec.form
        else {
            return Err(Error::InvalidSpec(
                "analytical surface binding targets a different form".to_owned(),
            ));
        };
        self.validate_molecule(molecule)?;
        if spec.structure_id() != Some(self.structure)
            || *method != self.method
            || *controls != self.controls
            || self.molecular_key != [molecule.identity(), molecule.coordinate_revision()]
        {
            return Err(Error::InvalidSpec(
                "analytical surface geometry contradicts its molecule, pose or scientific controls"
                    .to_owned(),
            ));
        }
        Ok(())
    }
}

/// Transportable scientific output; metadata describes the actual provider invocation.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct MolecularSurfaceData {
    /// Actual analytical provider.
    pub method: MolecularSurfaceMethod,
    /// Actual scientific controls.
    pub controls: MolecularSurfaceControls,
    /// Derived triangles, analytic normals and original atom owners.
    pub mesh: molgfx_core::MolecularSurfaceDescription,
}
