//! The interactive performance ladder: named scenes with their frame-rate budgets.

use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::{
    io,
    path::{Path, PathBuf},
};

/// Quality policy a ladder scene is measured under.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum LadderQuality {
    /// `profile::highest_fixed(120)`: maximum detail, never adapted.
    HighestFixed,
    /// `profile::adaptive(30)`: the engine may trade detail for the budget.
    Auto,
}

/// One ladder scene.
#[derive(Clone, Copy, Debug)]
pub struct LadderCase {
    /// Stable identifier, `L1`..`L8`.
    pub id: &'static str,
    /// Fixture id in `parity/corpus.json`.
    pub fixture: &'static str,
    /// Command text executed in order on the fixture's scene.
    pub commands: &'static [&'static str],
    /// Quality policy.
    pub quality: LadderQuality,
    /// Frames per second the case must sustain (p95 latency ≤ 1 / budget).
    pub budget_fps: u32,
}

/// The eight ladder scenes, measured at 1920×1080.
pub const LADDER: [LadderCase; 8] = [
    LadderCase {
        id: "L1",
        fixture: "4HHB",
        commands: &["pocket, resname HEM"],
        quality: LadderQuality::HighestFixed,
        budget_fps: 120,
    },
    LadderCase {
        id: "L2",
        fixture: "1AON",
        commands: &["show cartoon, protein"],
        quality: LadderQuality::HighestFixed,
        budget_fps: 120,
    },
    LadderCase {
        id: "L3",
        fixture: "1AON",
        commands: &["show spacefill, all"],
        quality: LadderQuality::HighestFixed,
        budget_fps: 120,
    },
    LadderCase {
        id: "L4",
        fixture: "lattice-100000",
        commands: &["show spacefill, all"],
        quality: LadderQuality::HighestFixed,
        budget_fps: 120,
    },
    LadderCase {
        id: "L5",
        fixture: "4HHB",
        commands: &[
            "show cartoon, protein",
            "show surface kind=solvent_excluded as shell, protein",
            "opacity 0.5, @shell",
        ],
        quality: LadderQuality::HighestFixed,
        budget_fps: 120,
    },
    LadderCase {
        id: "L6",
        fixture: "1AON",
        commands: &["show surface kind=solvent_excluded, protein"],
        quality: LadderQuality::HighestFixed,
        budget_fps: 120,
    },
    LadderCase {
        id: "L7",
        fixture: "lattice-1000000",
        commands: &["show spacefill, all"],
        quality: LadderQuality::HighestFixed,
        budget_fps: 60,
    },
    LadderCase {
        id: "L8",
        fixture: "lattice-10000000",
        commands: &["show spacefill, all"],
        quality: LadderQuality::Auto,
        budget_fps: 30,
    },
];

/// Looks a ladder scene up by id.
#[must_use]
pub fn find(id: &str) -> Option<&'static LadderCase> {
    LADDER.iter().find(|case| case.id == id)
}

#[derive(Deserialize)]
struct Corpus {
    fixtures: Vec<CorpusFixture>,
}

#[derive(Deserialize)]
struct CorpusFixture {
    id: String,
    file: String,
    sha256: String,
}

/// Resolves a corpus fixture file inside `cache`, verifying its SHA-256.
///
/// # Errors
/// Returns an error for an unknown fixture, an unreadable or altered file.
pub fn fixture_path(manifest: &Path, cache: &Path, id: &str) -> Result<PathBuf, io::Error> {
    let corpus: Corpus = serde_json::from_slice(&std::fs::read(manifest)?)?;
    let fixture = corpus
        .fixtures
        .into_iter()
        .find(|fixture| fixture.id == id)
        .ok_or_else(|| io::Error::other(format!("fixture {id} is not in the corpus manifest")))?;
    let path = cache.join(&fixture.file);
    let mut file = std::fs::File::open(&path)?;
    let mut hash = Sha256::new();
    io::copy(&mut file, &mut hash)?;
    if format!("{:x}", hash.finalize()) != fixture.sha256 {
        return Err(io::Error::other(format!("fixture {id} SHA-256 mismatch")));
    }
    Ok(path)
}
