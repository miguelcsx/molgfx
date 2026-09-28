//! Typed state for crystallographic, fitting, validation, and export surfaces.
//!
//! These values are deliberately renderer-independent.  Native lowering consumes
//! them through the existing core primitives; keeping the requests immutable
//! makes scene patches, snapshots, and host retries deterministic.

use crate::Error;
use crate::id::StructureId;
use serde::{Deserialize, Serialize};

/// A Cartesian unit cell supplied by crystallographic metadata.
#[derive(Clone, Copy, PartialEq, Debug, Serialize, Deserialize)]
pub struct UnitCellSpec {
    /// Cell lengths `[a, b, c]` in ångström.
    pub lengths: [f32; 3],
    /// Cell angles `[alpha, beta, gamma]` in degrees.
    pub angles_degrees: [f32; 3],
    /// Cartesian origin in ångström.
    pub origin: [f32; 3],
}

impl UnitCellSpec {
    /// Creates a validated unit-cell descriptor.
    ///
    /// # Errors
    ///
    /// Returns an error when a length or angle is non-finite or outside its
    /// valid range.
    pub fn new(lengths: [f32; 3], angles_degrees: [f32; 3]) -> Result<Self, Error> {
        let valid_lengths = lengths
            .iter()
            .all(|value| value.is_finite() && *value > 0.0);
        let valid_angles = angles_degrees
            .iter()
            .all(|value| value.is_finite() && *value > 0.0 && *value < 180.0);
        if !valid_lengths || !valid_angles {
            return Err(Error::InvalidSpec(
                "unit-cell parameters are invalid".to_owned(),
            ));
        }
        Ok(Self {
            lengths,
            angles_degrees,
            origin: [0.0; 3],
        })
    }

    /// Sets the Cartesian origin.
    ///
    /// # Errors
    ///
    /// Returns an error when the origin is not finite.
    ///
    /// # Errors
    ///
    /// Returns an error when the origin is not finite.
    pub fn with_origin(mut self, origin: [f32; 3]) -> Result<Self, Error> {
        if origin.iter().any(|value| !value.is_finite()) {
            return Err(Error::InvalidSpec(
                "unit-cell origin must be finite".to_owned(),
            ));
        }
        self.origin = origin;
        Ok(self)
    }
}

/// One stable assembly/symmetry instance, retaining source structure identity.
#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
pub struct AssemblyInstance {
    /// Caller-defined stable instance identity.
    pub id: u32,
    /// Structure supplying source atoms.
    pub structure: StructureId,
    /// Row-major affine transform in Cartesian space.
    pub transform: [f32; 16],
}

impl AssemblyInstance {
    /// Creates an instance after checking finite transform values.
    ///
    /// # Errors
    ///
    /// Returns an error when the transform is not finite.
    ///
    /// # Errors
    ///
    /// Returns an error when the transform is not finite.
    pub fn new(id: u32, structure: StructureId, transform: [f32; 16]) -> Result<Self, Error> {
        if transform.iter().any(|value| !value.is_finite()) {
            return Err(Error::InvalidSpec(
                "assembly transform must be finite".to_owned(),
            ));
        }
        let determinant = transform[0]
            * (transform[5] * transform[10] - transform[6] * transform[9])
            - transform[1] * (transform[4] * transform[10] - transform[6] * transform[8])
            + transform[2] * (transform[4] * transform[9] - transform[5] * transform[8]);
        if determinant.abs() <= f32::EPSILON {
            return Err(Error::InvalidSpec(
                "assembly transform must be invertible".to_owned(),
            ));
        }
        Ok(Self {
            id,
            structure,
            transform,
        })
    }
}

/// Assembly choice plus optional unit-cell guides.
#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
pub struct AssemblySpec {
    /// Structures to which the assembly applies.
    pub structures: Vec<StructureId>,
    /// Stable transformed instances, in deterministic draw order.
    pub instances: Vec<AssemblyInstance>,
    /// Optional source unit cell used for guides.
    pub unit_cell: Option<UnitCellSpec>,
}

impl AssemblySpec {
    /// Validates ownership and rejects duplicate instance identities.
    ///
    /// # Errors
    ///
    /// Returns an error when the assembly spec is invalid.
    ///
    /// # Errors
    ///
    /// Returns an error when the assembly spec is invalid.
    pub fn validate(&self) -> Result<(), Error> {
        let mut ids = std::collections::BTreeSet::new();
        let mut structures = std::collections::BTreeSet::new();
        for structure in &self.structures {
            if !structures.insert(*structure) {
                return Err(Error::InvalidSpec(
                    "assembly structure identity is duplicated".to_owned(),
                ));
            }
        }
        for instance in &self.instances {
            if !self.structures.contains(&instance.structure) || !ids.insert(instance.id) {
                return Err(Error::InvalidSpec(
                    "assembly instance identity or owner is invalid".to_owned(),
                ));
            }
        }
        Ok(())
    }
}

/// Result of a caller-owned structural fitting operation.
#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
pub struct FitResult {
    /// Sourcestructure structure used for correspondence.
    pub source: StructureId,
    /// Target structure used for correspondence.
    /// Target structure used for correspondence.
    pub target: StructureId,
    /// Number of accepted atom correspondences.
    pub correspondences: u64,
    /// RMSD in ångström.
    pub rmsd: f64,
    /// Row-major transform mapping source onto target.
    pub transform: [f32; 16],
    /// Provenance label for the fitting method and options.
    pub provenance: Box<str>,
}

impl FitResult {
    /// Validates a fitting result before it is retained in scene state.
    ///
    /// # Errors
    ///
    /// Returns an error when correspondence count, RMSD, transform, or
    /// provenance is invalid.
    pub fn validate(&self) -> Result<(), Error> {
        if self.correspondences == 0
            || !self.rmsd.is_finite()
            || self.rmsd < 0.0
            || self.transform.iter().any(|value| !value.is_finite())
            || self.provenance.trim().is_empty()
        {
            return Err(Error::InvalidSpec(
                "fitting result is incomplete or invalid".to_owned(),
            ));
        }
        Ok(())
    }
}

/// A caller-computed validation finding with display provenance.
#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
pub struct ValidationFinding {
    /// Structure containing the finding.
    pub structure: StructureId,
    /// Stable source row or semantic entity identifier.
    pub entity: u32,
    /// Finding category supplied by the validator.
    pub kind: Box<str>,
    /// Normalized severity in `[0, 1]`.
    pub severity: f32,
    /// Source method, version, and coordinate/topology identity.
    pub provenance: Box<str>,
}

impl ValidationFinding {
    /// Checks the portable finding contract.
    ///
    /// # Errors
    ///
    /// Returns an error when the finding is invalid.
    ///
    /// # Errors
    ///
    /// Returns an error when the finding is invalid.
    pub fn validate(&self) -> Result<(), Error> {
        if self.kind.trim().is_empty()
            || self.provenance.trim().is_empty()
            || !self.severity.is_finite()
            || !(0.0..=1.0).contains(&self.severity)
        {
            return Err(Error::InvalidSpec(
                "validation finding is invalid".to_owned(),
            ));
        }
        Ok(())
    }
}

/// Deterministic movie/export request. Rendering is performed by the native
/// export path; this type never substitutes a sequence of screenshots.
#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
pub struct MovieExportRequest {
    /// Inclusive trajectory frame range.
    pub first_frame: u64,
    /// Inclusive trajectory frame range end.
    pub last_frame: u64,
    /// Output dimensions in pixels.
    pub dimensions: [u32; 2],
    /// Output frames per second.
    pub frames_per_second: u32,
    /// Stable camera sampling seed.
    pub camera_seed: u64,
}

impl MovieExportRequest {
    /// Validates bounded deterministic export parameters.
    ///
    /// # Errors
    ///
    /// Returns an error when the request is invalid.
    ///
    /// # Errors
    ///
    /// Returns an error when the request is invalid.
    pub fn validate(&self) -> Result<(), Error> {
        if self.first_frame > self.last_frame
            || self.dimensions.contains(&0)
            || self.frames_per_second == 0
        {
            return Err(Error::InvalidSpec(
                "movie export request is invalid".to_owned(),
            ));
        }
        Ok(())
    }
}
