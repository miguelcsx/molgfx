//! The golden-image corpus: renders the reference scenes and diffs them against
//! the stored references (`docs/22-testing.md` §3, §5).
//!
//! Two tiers, as the contract requires. On the adapter the references were
//! captured on, a diff above tolerance is a **failure**. On any other adapter
//! the same diff is **advisory**: pixel determinism binds one adapter only
//! (`docs/18` §3), so drift elsewhere is a signal, not a verdict — and it is
//! reported, never silently passed. Adapter identity is a capability
//! fingerprint rather than a backend name, because nothing above the HAL is
//! allowed to know which backend it is talking to.
//!
//! Usage: `cargo run --release --example golden [--update]`

use pdviewx::{
    AtomSelection, Camera, ColorScheme, Engine, EngineConfig, Image, ImageConfig, RenderProfile,
    RepresentationKind, RepresentationPreset, RepresentationTarget, Scene, SurfaceKind,
    TubeRadiusMapping,
};
use std::error::Error;
use std::fs::{self, File};
use std::io;
use std::io::BufReader;
use std::path::{Path, PathBuf};

#[path = "common/mod.rs"]
mod common;

/// One corpus entry. Small on purpose: the corpus is a regression net, not a
/// gallery, and every scene here has to earn its render time.
struct GoldenScene {
    name: &'static str,
    structure: &'static str,
    kind: RepresentationKind,
    preset: Option<RepresentationPreset>,
    surface: Option<SurfaceKind>,
    /// Colour by chain rather than by element, so a change in the chain palette
    /// shows up as a changed picture too.
    by_chain: bool,
    cinematic: bool,
}

/// The corpus. Each entry covers a distinct geometry path — impostors, splines,
/// nucleotide slabs, marching-cubes surface, glycosidic ribbons — so a break in
/// any one of them shows up as a changed picture.
const CORPUS: [GoldenScene; 13] = [
    GoldenScene {
        name: "ubiquitin-spacefill",
        structure: "1ubq.cif",
        kind: RepresentationKind::Spacefill,
        preset: None,
        surface: None,
        by_chain: false,
        cinematic: false,
    },
    GoldenScene {
        name: "haemoglobin-cartoon",
        structure: "4hhb.cif",
        kind: RepresentationKind::Cartoon,
        preset: None,
        surface: None,
        by_chain: true,
        cinematic: true,
    },
    GoldenScene {
        name: "dna-cartoon",
        structure: "1BNA.cif",
        kind: RepresentationKind::Cartoon,
        preset: None,
        surface: None,
        by_chain: false,
        cinematic: false,
    },
    GoldenScene {
        name: "trypsin-ball-and-stick",
        structure: "3PTB.cif",
        kind: RepresentationKind::BallAndStick,
        preset: None,
        surface: None,
        by_chain: false,
        cinematic: false,
    },
    GoldenScene {
        name: "trypsin-round-wires",
        structure: "3PTB.cif",
        kind: RepresentationKind::Lines,
        preset: None,
        surface: None,
        by_chain: false,
        cinematic: false,
    },
    GoldenScene {
        name: "ubiquitin-points",
        structure: "1ubq.cif",
        kind: RepresentationKind::Points,
        preset: None,
        surface: None,
        by_chain: true,
        cinematic: false,
    },
    GoldenScene {
        name: "ubiquitin-residue-beads",
        structure: "1ubq.cif",
        kind: RepresentationKind::Beads,
        preset: None,
        surface: None,
        by_chain: true,
        cinematic: false,
    },
    GoldenScene {
        name: "haemoglobin-rocket",
        structure: "4hhb.cif",
        kind: RepresentationKind::Rocket,
        preset: None,
        surface: None,
        by_chain: true,
        cinematic: true,
    },
    GoldenScene {
        name: "haemoglobin-putty",
        structure: "4hhb.cif",
        kind: RepresentationKind::Tube,
        preset: None,
        surface: None,
        by_chain: true,
        cinematic: false,
    },
    GoldenScene {
        name: "ubiquitin-surface",
        structure: "1ubq.cif",
        kind: RepresentationKind::Surface,
        preset: None,
        surface: Some(SurfaceKind::SolventExcluded),
        by_chain: false,
        cinematic: false,
    },
    GoldenScene {
        name: "spike-twister",
        structure: "6VXX.cif",
        kind: RepresentationKind::Twister,
        preset: None,
        surface: None,
        by_chain: true,
        cinematic: true,
    },
    GoldenScene {
        name: "dna-paper-chain",
        structure: "1BNA.cif",
        kind: RepresentationKind::PaperChain,
        preset: Some(RepresentationPreset::PaperChain),
        surface: None,
        by_chain: true,
        cinematic: false,
    },
    GoldenScene {
        name: "ubiquitin-dotted-solvent",
        structure: "1ubq.cif",
        kind: RepresentationKind::Surface,
        preset: Some(RepresentationPreset::DottedSolvent),
        surface: None,
        by_chain: false,
        cinematic: false,
    },
];

/// Reference resolution. Small enough that the corpus stays cheap to store and
/// re-render, large enough that a geometry regression is visible.
const WIDTH: u32 = 320;
const HEIGHT: u32 = 240;
/// The same ratio as the dimensions above, written out so the camera needs no
/// integer-to-float cast.
const ASPECT: f32 = 320.0 / 240.0;
/// Per-channel difference a pixel may carry before it counts as changed. Below
/// this, a difference is invisible next to the tonemap's own quantisation.
const CHANNEL_TOLERANCE: u8 = 8;
/// Fraction of changed pixels a scene may carry and still pass.
const PIXEL_TOLERANCE: f64 = 0.005;

fn main() -> Result<(), Box<dyn Error>> {
    let update = std::env::args().any(|argument| argument == "--update");
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let golden = root.join("benchmarks/golden");
    fs::create_dir_all(&golden)?;

    let fingerprint_path = golden.join("adapter.txt");
    let mut failures = 0usize;
    let mut advisories = 0usize;
    let mut fingerprint = String::new();

    for scene in &CORPUS {
        let (image, capabilities) = match render(&root, scene) {
            Ok(rendered) => rendered,
            // No conformant GPU here: the level is skipped with a note rather
            // than passed silently (`docs/22` §3).
            Err(RunError::NoAdapter) => {
                println!("GOLDEN: SKIPPED — no conformant adapter on this machine");
                return Ok(());
            }
            Err(RunError::Failed(error)) => return Err(error),
        };
        fingerprint = capabilities;
        let reference_path = golden.join(format!("{}.png", scene.name));
        if update || !reference_path.exists() {
            write_png(&reference_path, &image)?;
            println!("{:<26} captured", scene.name);
            continue;
        }
        let reference = read_png(&reference_path)?;
        let changed = compare(&image, &reference);
        let verdict = match changed {
            Some(changed) if changed > PIXEL_TOLERANCE => {
                let diff = golden.join(format!("{}.diff.png", scene.name));
                write_png(&diff, &difference_image(&image, &reference))?;
                failures += 1;
                format!(
                    "{:.3}% changed, diff at {}",
                    changed * 100.0,
                    diff.display()
                )
            }
            Some(changed) => format!("{changed:.5}% changed"),
            None => {
                failures += 1;
                "reference has a different size".to_owned()
            }
        };
        println!("{:<26} {verdict}", scene.name);
    }

    if update {
        fs::write(&fingerprint_path, &fingerprint)?;
        println!("captured {} references", CORPUS.len());
        return Ok(());
    }

    // The tier: the same numbers, a different verdict.
    let binding = fs::read_to_string(&fingerprint_path)
        .is_ok_and(|recorded| !recorded.trim().is_empty() && recorded.trim() == fingerprint.trim());
    if !binding {
        advisories = failures;
        failures = 0;
    }
    if advisories > 0 {
        println!(
            "\n{advisories} scene(s) drifted. ADVISORY: this adapter is not the one the \
             references were captured on, so the diff is a signal and not a verdict. \
             Re-run on the reference adapter to get a binding result."
        );
        return Ok(());
    }
    if failures > 0 {
        return Err(io::Error::other(format!("{failures} golden scene(s) drifted")).into());
    }
    println!(
        "\nGOLDEN: OK ({} scenes, {})",
        CORPUS.len(),
        if binding { "binding" } else { "advisory" }
    );
    Ok(())
}

/// Renders one scene twice and returns the image only when both runs agree,
/// which is the determinism assertion of `docs/22` §5 — a renderer that cannot
/// reproduce its own frame cannot have a golden image at all.
/// Why a scene did not render: an absent adapter is a skip, anything else is a
/// failure.
enum RunError {
    NoAdapter,
    Failed(Box<dyn Error>),
}

impl<E: Into<Box<dyn Error>>> From<E> for RunError {
    fn from(error: E) -> Self {
        Self::Failed(error.into())
    }
}

fn render(root: &Path, scene: &GoldenScene) -> Result<(Image, String), RunError> {
    let profile = if scene.cinematic {
        RenderProfile::cinematic()
    } else {
        RenderProfile::inspection()
    };
    let engine = Engine::new(
        &EngineConfig {
            profile,
            ..EngineConfig::default()
        },
        None,
    );
    let mut engine = match engine {
        Ok(engine) => engine,
        Err(pdviewx::RenderError::Gpu(pdviewx::GpuError::NoAdapter)) => {
            return Err(RunError::NoAdapter);
        }
        Err(error) => return Err(RunError::Failed(error.into())),
    };
    let capabilities = format!("{:?}", engine.capabilities());
    let path = root.join("benchmarks/scenes").join(scene.structure);
    let Some(path) = path.to_str() else {
        return Err(io::Error::other("scene path is not valid UTF-8").into());
    };
    let structure = common::read_structure(path)?;
    let mut built = Scene::from_structure(&structure)?;
    let first = built.structures().next().map(|(handle, _)| handle);
    if let Some(handle) = first {
        let records = common::deposited_secondary_structure(path, &structure);
        if !records.is_empty() {
            built.apply_secondary_structure(handle, &records)?;
        }
    }
    let selection = built.add_selection(AtomSelection::All);
    let representation = if let Some(preset) = scene.preset {
        built
            .represent_preset(RepresentationTarget::Selection(selection), preset)?
            .into_iter()
            .next()
            .ok_or_else(|| io::Error::other("preset produced no representation"))?
    } else {
        built.represent(selection, scene.kind)?
    };
    if let Some(representation) = built.representation_mut(representation) {
        if let Some(surface) = scene.surface {
            representation.params.surface_kind = surface;
        }
        if scene.by_chain {
            representation.color = ColorScheme::ByChain;
        }
        if scene.kind == RepresentationKind::Tube {
            representation.params.tube_radius_mapping =
                TubeRadiusMapping::b_factor([0.0, 80.0], [0.12, 0.72])?;
        }
    }
    let config = ImageConfig {
        width: WIDTH,
        height: HEIGHT,
    };
    let bounds = built.world_aabb();
    let camera = Camera::framing_aabb(&bounds, ASPECT);

    let image = engine.render_image(&built, &camera, config)?;
    let repeat = engine.render_image(&built, &camera, config)?;
    if image.pixels != repeat.pixels {
        return Err(RunError::Failed(Box::new(io::Error::other(format!(
            "{}: two renders of one scene disagree, so no reference is meaningful",
            scene.name
        )))));
    }
    Ok((image, capabilities))
}

/// The fraction of pixels that changed beyond `CHANNEL_TOLERANCE`, or `None`
/// when the images are not the same size.
fn compare(image: &Image, reference: &Image) -> Option<f64> {
    if image.width != reference.width || image.height != reference.height {
        return None;
    }
    // Counted as floats so the ratio needs no integer-to-float cast.
    let mut changed = 0.0f64;
    let mut total = 0.0f64;
    for (pixel, other) in image
        .pixels
        .chunks_exact(4)
        .zip(reference.pixels.chunks_exact(4))
    {
        total += 1.0;
        if pixel
            .iter()
            .zip(other)
            .any(|(left, right)| left.abs_diff(*right) > CHANNEL_TOLERANCE)
        {
            changed += 1.0;
        }
    }
    Some(changed / total.max(1.0))
}

/// A diff image: what changed, in red, over a dimmed copy of the new render —
/// so the failure is readable without loading both files side by side.
fn difference_image(image: &Image, reference: &Image) -> Image {
    let mut pixels = Vec::with_capacity(image.pixels.len());
    for (pixel, other) in image
        .pixels
        .chunks_exact(4)
        .zip(reference.pixels.chunks_exact(4))
    {
        let delta = pixel
            .iter()
            .zip(other)
            .map(|(left, right)| left.abs_diff(*right))
            .max()
            .map_or(0, |delta| delta);
        let (Some(&red), Some(&green), Some(&blue)) = (pixel.first(), pixel.get(1), pixel.get(2))
        else {
            continue;
        };
        if delta > CHANNEL_TOLERANCE {
            pixels.extend_from_slice(&[255, 32, 32, 255]);
        } else {
            let dim = |value: u8| value / 3 + 160;
            pixels.extend_from_slice(&[dim(red), dim(green), dim(blue), 255]);
        }
    }
    Image {
        width: image.width,
        height: image.height,
        pixels,
    }
}

fn write_png(path: &PathBuf, image: &Image) -> Result<(), Box<dyn Error>> {
    let mut encoder = png::Encoder::new(File::create(path)?, image.width, image.height);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    encoder.write_header()?.write_image_data(&image.pixels)?;
    Ok(())
}

fn read_png(path: &PathBuf) -> Result<Image, Box<dyn Error>> {
    let decoder = png::Decoder::new(BufReader::new(File::open(path)?));
    let mut reader = decoder.read_info()?;
    let Some(size) = reader.output_buffer_size() else {
        return Err(io::Error::other("reference PNG declares no buffer size").into());
    };
    let mut pixels = vec![0; size];
    let info = reader.next_frame(&mut pixels)?;
    pixels.truncate(info.buffer_size());
    Ok(Image {
        width: info.width,
        height: info.height,
        pixels,
    })
}
