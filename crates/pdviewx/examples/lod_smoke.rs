//! Deterministic visual audit of the biological LOD ladder.

use pdviewx::{
    AtomSelection, Camera, Engine, EngineConfig, Image, ImageConfig, LodFrame, LodIndex, LodLevel,
    LodPolicy, LodScene, RepresentationKind, Scene,
};
use std::error::Error;
use std::fs::File;
use std::io;
use std::path::Path;

#[path = "common/mod.rs"]
mod common;

const IMAGE: ImageConfig = ImageConfig {
    width: 1024,
    height: 768,
};
const ASPECT: f32 = 4.0 / 3.0;

fn main() -> Result<(), Box<dyn Error>> {
    let arguments = std::env::args().skip(1).collect::<Vec<_>>();
    let source = arguments
        .first()
        .map_or("benchmarks/scenes/6VXX.cif", String::as_str);
    let prefix = arguments
        .get(1)
        .map_or("target/visual-checks/lod", String::as_str);
    let structure = common::read_structure(source)?;
    let mut scene = Scene::from_structure(&structure)?;
    let Some((owner, _)) = scene.structures().next() else {
        return Err(io::Error::other("LOD scene has no placed structure").into());
    };
    let secondary_structure = common::deposited_secondary_structure(source, &structure);
    if !secondary_structure.is_empty() {
        scene.apply_secondary_structure(owner, &secondary_structure)?;
    }
    let selection = scene.add_selection(AtomSelection::All);
    let native = scene.represent(selection, RepresentationKind::Spacefill)?;
    let selection_camera = Camera::framing_aabb(&scene.world_aabb(), ASPECT);
    let index = LodIndex::from_scene(&scene);
    let mut lod_scene = LodScene::default();
    lod_scene.bind_detail_representation(&scene, owner, native)?;
    let mut engine = Engine::new(&EngineConfig::default(), None)?;

    let mut atom = LodFrame::default();
    index.select_into(
        &selection_camera,
        viewport(),
        policy(LodLevel::Atom),
        None,
        &mut atom,
    );
    let mut residue = LodFrame::default();
    index.select_into(
        &selection_camera,
        viewport(),
        policy(LodLevel::Residue),
        None,
        &mut residue,
    );
    let mut secondary = LodFrame::default();
    index.select_into(
        &selection_camera,
        viewport(),
        policy(LodLevel::SecondaryStructure),
        None,
        &mut secondary,
    );
    let mut domain = LodFrame::default();
    index.select_into(
        &selection_camera,
        viewport(),
        policy(LodLevel::Domain),
        None,
        &mut domain,
    );

    let mut audit = LodAudit {
        scene: &mut scene,
        index: &index,
        lod_scene: &mut lod_scene,
        engine: &mut engine,
        prefix,
    };
    audit.render_level("atom", &atom)?;
    audit.render_level("residue", &residue)?;
    audit.render_level("secondary", &secondary)?;
    audit.render_level("domain", &domain)?;
    audit.render_transition(&atom, &residue)?;
    audit.render_transition(&residue, &secondary)?;
    Ok(())
}

struct LodAudit<'a> {
    scene: &'a mut Scene,
    index: &'a LodIndex,
    lod_scene: &'a mut LodScene,
    engine: &'a mut Engine,
    prefix: &'a str,
}

impl LodAudit<'_> {
    fn render_level(&mut self, label: &str, frame: &LodFrame) -> Result<(), Box<dyn Error>> {
        self.lod_scene.apply(self.scene, self.index, frame)?;
        let camera = Camera::framing_aabb(&self.scene.world_aabb(), ASPECT);
        let image = self.engine.render_image(self.scene, &camera, IMAGE)?;
        let repeat = self.engine.render_image(self.scene, &camera, IMAGE)?;
        if !within_one_lsb(&image, &repeat) {
            return Err(io::Error::other(format!(
                "{label} LOD output exceeds one-LSB raster tolerance"
            ))
            .into());
        }
        write_png(format!("{}-{label}.png", self.prefix), &image)?;
        println!(
            "{label}: {} visible clusters, {} persistent coarse records",
            frame.visible().len(),
            self.lod_scene.primitive_count()
        );
        Ok(())
    }

    fn render_transition(&mut self, from: &LodFrame, to: &LodFrame) -> Result<(), Box<dyn Error>> {
        self.lod_scene
            .apply_transition(self.scene, self.index, from, to, 0.5)?;
        let camera = Camera::framing_aabb(&self.scene.world_aabb(), ASPECT);
        let image = self.engine.render_image(self.scene, &camera, IMAGE)?;
        let repeat = self.engine.render_image(self.scene, &camera, IMAGE)?;
        if !within_one_lsb(&image, &repeat) {
            return Err(
                io::Error::other("transition LOD output exceeds one-LSB blend tolerance").into(),
            );
        }
        let from_level = from
            .levels()
            .next()
            .map_or("empty", |(_, level)| level_name(level));
        let to_level = to
            .levels()
            .next()
            .map_or("empty", |(_, level)| level_name(level));
        write_png(
            format!("{}-transition-{from_level}-{to_level}.png", self.prefix),
            &image,
        )?;
        println!(
            "transition {from_level}->{to_level}: {} persistent coarse records",
            self.lod_scene.primitive_count()
        );
        Ok(())
    }
}

fn within_one_lsb(left: &Image, right: &Image) -> bool {
    left.width == right.width
        && left.height == right.height
        && left.pixels.len() == right.pixels.len()
        && left
            .pixels
            .iter()
            .zip(&right.pixels)
            .all(|(first, second)| first.abs_diff(*second) <= 1)
}

const fn viewport() -> [u32; 2] {
    [IMAGE.width, IMAGE.height]
}

const fn level_name(level: LodLevel) -> &'static str {
    match level {
        LodLevel::Atom => "atom",
        LodLevel::Residue => "residue",
        LodLevel::SecondaryStructure => "secondary",
        LodLevel::Domain => "domain",
    }
}

const fn policy(level: LodLevel) -> LodPolicy {
    let maximum = f32::MAX;
    match level {
        LodLevel::Atom => LodPolicy {
            atom_pixels: 0.0,
            residue_pixels: 0.0,
            secondary_pixels: 0.0,
            hysteresis: 0.0,
        },
        LodLevel::Residue => LodPolicy {
            atom_pixels: maximum,
            residue_pixels: 0.0,
            secondary_pixels: 0.0,
            hysteresis: 0.0,
        },
        LodLevel::SecondaryStructure => LodPolicy {
            atom_pixels: maximum,
            residue_pixels: maximum,
            secondary_pixels: 0.0,
            hysteresis: 0.0,
        },
        LodLevel::Domain => LodPolicy {
            atom_pixels: maximum,
            residue_pixels: maximum,
            secondary_pixels: maximum,
            hysteresis: 0.0,
        },
    }
}

fn write_png(path: impl AsRef<Path>, image: &Image) -> Result<(), Box<dyn Error>> {
    let path = path.as_ref();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let mut encoder = png::Encoder::new(File::create(path)?, image.width, image.height);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    encoder.write_header()?.write_image_data(&image.pixels)?;
    Ok(())
}
