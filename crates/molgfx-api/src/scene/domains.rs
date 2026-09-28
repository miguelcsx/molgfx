//! Typed authoring state for crystallographic instances, fitting, validation and export.
//!
//! These values are renderer-independent.  Renderers may consume them, but do
//! not own the identity or the deterministic sampling policy.

use crate::{SceneSpec, StructureId};
use serde::{Deserialize, Serialize};

/// A crystallographic assembly selected for one source structure.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AssemblyChoice {
    /// Source structure for the selected assembly.
    pub structure: StructureId,
    /// Stable source assembly identifier.
    pub assembly_id: Box<str>,
}

/// One placed symmetry/assembly instance. `matrix` is column-major affine.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct StructureInstance {
    /// Source structure supplying the instance.
    pub structure: StructureId,
    /// Optional source assembly identifier.
    pub assembly_id: Option<Box<str>>,
    /// Stable symmetry/operator identifier.
    pub operator_id: Box<str>,
    /// Column-major affine transform.
    pub matrix: [f32; 16],
}

/// Crystallographic unit-cell parameters and fractional-to-cartesian transform.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct UnitCell {
    /// Cell lengths along the three axes.
    pub lengths: [f64; 3],
    /// Cell angles in degrees.
    pub angles: [f64; 3],
    /// Fractional-to-cartesian column-major transform.
    pub fractional_to_cartesian: [[f64; 4]; 4],
}

/// Result of a native correspondence/fitting operation.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct FittingResult {
    /// Reference structure used by the fit.
    pub reference: StructureId,
    /// Mobile structure transformed by the fit.
    pub mobile: StructureId,
    /// Number of matched records.
    pub correspondences: u32,
    /// Root-mean-square deviation, when computed.
    pub rmsd: Option<f64>,
    /// Column-major fitting transform.
    pub transform: [[f64; 4]; 4],
    /// Fitting method identifier.
    pub method: Box<str>,
}

/// A validation colour attached to one source row or residue.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ValidationColor {
    /// Structure containing the validated row.
    pub structure: StructureId,
    /// Source row or residue index.
    pub row: u32,
    /// Display colour in red, green, blue, alpha order.
    pub rgba: [u8; 4],
    /// Optional validator score.
    pub score: Option<i32>,
    /// Validation source identifier.
    pub source: Box<str>,
}

/// A deterministic movie/export request. No screenshot fallback is implied.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct MovieExportRequest {
    /// Inclusive first frame.
    pub start_frame: u32,
    /// Inclusive last frame.
    pub end_frame: u32,
    /// Output frame rate in frames per second.
    pub frame_rate: f64,
    /// Output width in pixels.
    pub width: u32,
    /// Output height in pixels.
    pub height: u32,
    /// Requested output format.
    pub output_format: Box<str>,
    /// Optional destination URI.
    pub output_uri: Option<Box<str>>,
}

impl MovieExportRequest {
    /// Validates the request without touching a renderer or filesystem.
    ///
    /// # Errors
    ///
    /// Returns an error when the request is invalid.
    ///
    /// # Errors
    ///
    /// Returns an error when the request is invalid.
    ///
    /// # Errors
    ///
    /// Returns an error when the request is invalid.
    pub fn validate(&self) -> Result<(), String> {
        if self.end_frame < self.start_frame {
            return Err("movie end_frame precedes start_frame".to_owned());
        }
        if !self.frame_rate.is_finite() || self.frame_rate <= 0.0 {
            return Err("movie frame_rate must be finite and positive".to_owned());
        }
        if self.width == 0 || self.height == 0 {
            return Err("movie dimensions must be non-zero".to_owned());
        }
        if self.output_format.is_empty() {
            return Err("movie output_format must not be empty".to_owned());
        }
        Ok(())
    }

    /// Returns presentation times in stable frame order.
    ///
    /// # Errors
    ///
    /// Returns an error when the export request is invalid.
    pub fn sample_times(&self) -> Result<Vec<f64>, String> {
        self.validate()?;
        Ok((self.start_frame..=self.end_frame)
            .map(|frame| f64::from(frame) / self.frame_rate)
            .collect())
    }
}

/// Portable retained snapshot, including the immutable semantic scene.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SceneSnapshot {
    /// Immutable semantic scene to restore.
    pub scene: SceneSpec,
    /// Placed structure instances retained with the scene.
    pub instances: Vec<StructureInstance>,
    /// Optional crystallographic unit-cell metadata.
    pub unit_cell: Option<UnitCell>,
    /// Optional structural fitting result.
    pub fitting: Option<FittingResult>,
    /// Caller-computed validation colours.
    pub validation: Vec<ValidationColor>,
}

impl SceneSnapshot {
    /// Creates a new snapshot from the given scene specification.
    ///
    /// # Returns
    ///
    /// A new `SceneSnapshot` with the scene specification and default values for other fields.
    ///
    /// # Errors
    ///
    /// Returns an error if the scene specification is invalid.
    #[must_use]
    pub fn from_scene(scene: &SceneSpec) -> Self {
        Self {
            scene: scene.clone(),
            instances: Vec::new(),
            unit_cell: None,
            fitting: None,
            validation: Vec::new(),
        }
    }

    /// Restores the semantic scene after validating all retained references.
    ///
    /// # Errors
    ///
    /// Returns an error when retained instances, fitting records, or validation
    /// colours reference unknown structures or contain inconsistent values.
    pub fn restore(self) -> Result<SceneSpec, String> {
        for instance in &self.instances {
            if !self.scene.structures.contains_key(&instance.structure) {
                return Err(format!(
                    "instance references unknown structure {}",
                    instance.structure.get()
                ));
            }
        }
        if let Some(fitting) = &self.fitting {
            if !self.scene.structures.contains_key(&fitting.reference)
                || !self.scene.structures.contains_key(&fitting.mobile)
            {
                return Err("fitting references an unknown structure".to_owned());
            }
            if fitting.correspondences == 0 && fitting.rmsd.is_some() {
                return Err("fitting with zero correspondences cannot have RMSD".to_owned());
            }
        }
        for color in &self.validation {
            if !self.scene.structures.contains_key(&color.structure) {
                return Err("validation colour references an unknown structure".to_owned());
            }
        }
        Ok(self.scene)
    }
}

#[cfg(test)]
mod tests {
    use super::MovieExportRequest;

    #[test]
    fn movie_sampling_is_inclusive_and_deterministic() {
        let request = MovieExportRequest {
            start_frame: 2,
            end_frame: 4,
            frame_rate: 2.0,
            width: 16,
            height: 16,
            output_format: "mp4".into(),
            output_uri: None,
        };
        assert_eq!(request.sample_times().unwrap(), vec![1.0, 1.5, 2.0]);
    }
}
