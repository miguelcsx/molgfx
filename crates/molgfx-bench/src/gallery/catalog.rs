//! Portable corpus contracts and content verification.
use super::checklist::CHECKLIST_IDS;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::{
    error::Error,
    fs::File,
    io::{self, Read},
    path::Path,
};

/// Harness errors retain their original parser, source or rendering diagnostics.
pub type Result<T> = std::result::Result<T, Box<dyn Error>>;

#[derive(Clone, Debug, Deserialize, Serialize)]
/// Complete gallery configuration and licensed fixture catalog.
pub struct Catalog {
    /// Physical output width and height.
    pub extent: [u32; 2],
    /// Completed outputs discarded before measurement.
    pub warmup_outputs: usize,
    /// Completed outputs included in the measurement distribution.
    pub measured_outputs: usize,
    /// Molecular style used by non-script cases.
    pub style: Style,
    /// Available native and reference recipe names.
    pub recipes: Vec<String>,
    /// Content-addressed scene inputs and authoring recipes.
    pub fixtures: Vec<Fixture>,
    /// Additional manifest metadata preserved through serialization.
    #[serde(flatten)]
    pub details: std::collections::BTreeMap<String, Value>,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
/// Shared molecular representation defaults for simple gallery cases.
pub struct Style {
    /// Uniform sRGB color.
    pub color_rgb: [u8; 3],
    /// Multiplier of physical atom radii.
    pub atom_radius_scale: f32,
    /// Bond radius in angstroms.
    pub bond_radius_angstrom: f32,
    /// Cartoon width in angstroms.
    pub cartoon_width_angstrom: f32,
    /// Representation opacity.
    pub opacity: f32,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
/// Licensed input, explicit camera and scene recipe for one gallery case.
pub struct Fixture {
    /// Stable case identifier.
    pub id: String,
    /// Input filename relative to the corpus cache.
    pub file: String,
    /// Source format understood by the canonical reader.
    pub format: String,
    /// Expected SHA-256 of the primary input bytes.
    pub sha256: String,
    /// Input license identifier.
    pub license: String,
    /// Authoritative source of the input license.
    pub license_url: String,
    /// Molecular selection supported by the simple-form recipe.
    pub selection: String,
    /// Molecular, volume or script recipe name.
    pub form: String,
    /// Explicit camera shared by all rendering recipes.
    pub camera: Camera,
    /// Optional command-based authoring recipe.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub script: Option<Script>,
    /// Reference recipes this case cannot render faithfully, with a specific reason.
    #[serde(default, skip_serializing_if = "std::collections::BTreeMap::is_empty")]
    pub omissions: std::collections::BTreeMap<String, String>,
    /// Domain-specific recipe arguments and provenance.
    #[serde(flatten)]
    pub details: std::collections::BTreeMap<String, Value>,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
/// Explicit perspective camera in world coordinates.
pub struct Camera {
    /// Eye position.
    pub position: [f32; 3],
    /// Look-at target.
    pub target: [f32; 3],
    /// Camera up direction.
    pub up: [f32; 3],
    /// Vertical field of view in degrees.
    pub fov_y_degrees: f32,
    /// Near clipping distance.
    pub near: f32,
    /// Far clipping distance.
    pub far: f32,
}
/// Command lines, reference-engine equivalents and review items for a gallery case.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Script {
    /// Native authoring commands executed in order.
    pub molgfx: Vec<String>,
    /// Equivalent `PyMOL` commands where supported.
    pub pymol: Vec<String>,
    /// Equivalent Mol* actions where supported.
    pub molstar: Vec<Value>,
    /// Required semantic and visual review items.
    pub checklist: Vec<String>,
    /// Optional selection-based camera fit.
    pub fit: Option<FitCamera>,
    /// Optional animated evidence recipe.
    pub video: Option<Video>,
}
/// Camera fitted to a selection's bounding sphere, looking down -Z with +Y up.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct FitCamera {
    /// Selection whose bounds determine the camera.
    pub selection: String,
    /// Vertical field of view in degrees.
    pub fov_y_degrees: f32,
    /// Bounding-sphere radius multiplier.
    pub margin: f32,
}
#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
/// Source of motion in an animated gallery case.
pub enum VideoKind {
    /// One full camera orbit around the world Y axis.
    OrbitY,
    /// Coordinate interpolation across source models.
    Trajectory,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
/// Animated gallery duration and sampling recipe.
pub struct Video {
    /// Camera or molecular motion.
    pub kind: VideoKind,
    /// Number of output images.
    pub frames: u32,
    /// Playback sampling rate.
    pub fps: u32,
}
impl Fixture {
    /// Validates the case contract, licensed source and primary input hash.
    ///
    /// # Errors
    /// Rejects unsupported authoring shapes, review omissions and corrupt inputs.
    pub fn verify(&self, cache: &Path) -> Result<()> {
        if self.license.is_empty() || self.license_url.is_empty() || self.selection != "all" {
            return Err(
                io::Error::other("fixture requires license and supported all selection").into(),
            );
        }
        match (self.form == "script", &self.script) {
            (true, Some(script)) => {
                if let Some(id) = script
                    .checklist
                    .iter()
                    .find(|id| !CHECKLIST_IDS.contains(&id.as_str()))
                {
                    return Err(io::Error::other(format!(
                        "case {} names unknown checklist id {id}",
                        self.id
                    ))
                    .into());
                }
            }
            (true, None) => {
                return Err(
                    io::Error::other(format!("script case {} has no script", self.id)).into(),
                );
            }
            (false, Some(_)) => {
                return Err(io::Error::other(format!(
                    "case {} has a script but form is not \"script\"",
                    self.id
                ))
                .into());
            }
            (false, None) => {}
        }
        if self
            .omissions
            .iter()
            .any(|(recipe, reason)| recipe.starts_with("molgfx-") || reason.trim().is_empty())
        {
            return Err(io::Error::other(format!(
                "fixture {} requires a reason for every omitted external recipe and cannot omit MolGFX",
                self.id
            ))
            .into());
        }
        let mut file = File::open(cache.join(&self.file))?;
        let mut hash = Sha256::new();
        let mut block = [0_u8; 16384];
        loop {
            let count = file.read(&mut block)?;
            if count == 0 {
                break;
            }
            hash.update(&block[..count]);
        }
        if format!("{:x}", hash.finalize()) != self.sha256 {
            return Err(io::Error::other(format!("fixture {} SHA-256 mismatch", self.id)).into());
        }
        Ok(())
    }
}

#[cfg(test)]
#[path = "catalog_tests.rs"]
mod tests;
