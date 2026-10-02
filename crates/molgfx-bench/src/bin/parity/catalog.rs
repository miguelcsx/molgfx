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

pub(super) type Result<T> = std::result::Result<T, Box<dyn Error>>;

#[derive(Clone, Deserialize, Serialize)]
pub(super) struct Catalog {
    pub extent: [u32; 2],
    pub warmup_outputs: usize,
    pub measured_outputs: usize,
    pub style: Style,
    pub recipes: Vec<String>,
    pub fixtures: Vec<Fixture>,
    #[serde(flatten)]
    pub details: std::collections::BTreeMap<String, Value>,
}
#[derive(Clone, Deserialize, Serialize)]
pub(super) struct Style {
    pub color_rgb: [u8; 3],
    pub atom_radius_scale: f32,
    pub bond_radius_angstrom: f32,
    pub cartoon_width_angstrom: f32,
    pub opacity: f32,
}
#[derive(Clone, Deserialize, Serialize)]
pub(super) struct Fixture {
    pub id: String,
    pub file: String,
    pub format: String,
    pub sha256: String,
    pub license: String,
    pub license_url: String,
    pub selection: String,
    pub form: String,
    pub camera: Camera,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub script: Option<Script>,
    #[serde(flatten)]
    pub details: std::collections::BTreeMap<String, Value>,
}
#[derive(Clone, Deserialize, Serialize)]
pub(super) struct Camera {
    pub position: [f32; 3],
    pub target: [f32; 3],
    pub up: [f32; 3],
    pub fov_y_degrees: f32,
    pub near: f32,
    pub far: f32,
}
/// Command lines, reference-engine equivalents and review items for a gallery case.
#[derive(Clone, Deserialize, Serialize)]
pub(super) struct Script {
    pub molgfx: Vec<String>,
    pub pymol: Vec<String>,
    pub molstar: Vec<Value>,
    pub checklist: Vec<String>,
    pub fit: Option<FitCamera>,
    pub video: Option<Video>,
}
/// Camera fitted to a selection's bounding sphere, looking down -Z with +Y up.
#[derive(Clone, Deserialize, Serialize)]
pub(super) struct FitCamera {
    pub selection: String,
    pub fov_y_degrees: f32,
    pub margin: f32,
}
#[derive(Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub(super) enum VideoKind {
    OrbitY,
    Trajectory,
}
#[derive(Clone, Deserialize, Serialize)]
pub(super) struct Video {
    pub kind: VideoKind,
    pub frames: u32,
    pub fps: u32,
}
impl Fixture {
    pub(super) fn verify(&self, cache: &Path) -> Result<()> {
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
