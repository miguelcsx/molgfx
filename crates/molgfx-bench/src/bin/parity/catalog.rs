//! Portable corpus contracts and content verification.
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
impl Fixture {
    pub(super) fn verify(&self, cache: &Path) -> Result<()> {
        if self.license.is_empty() || self.license_url.is_empty() || self.selection != "all" {
            return Err(
                io::Error::other("fixture requires license and supported all selection").into(),
            );
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
