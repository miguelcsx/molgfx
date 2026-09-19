//! Scene resolution for the benchmark harness.
//!
//! The only module in this crate that names the corpus directory, so the
//! harness can be pointed at any corpus without editing a declaration: a scene
//! arrives either as a path the caller typed or as a bare name from the
//! declarative `default_fixture` table, and this turns that into one path.
//!
//! A checkout with no corpus is a normal state, not a broken one: resolution
//! answers `None` only when nothing was asked for, and the caller decides
//! whether an absent file blocks the scene or the whole run.
//! `$MOLGFX_SCENES` points a run at another directory.

use std::path::{Path, PathBuf};

/// Directory the scene corpus lives in, relative to the repository root.
const SCENES: &str = "benchmarks/scenes";

/// Repository root, taken from the compile-time manifest directory so that
/// resolution never depends on the working directory.
fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// Directory that scenes are read from.
///
/// `$MOLGFX_SCENES` when set, otherwise the checkout's own directory.
#[must_use]
pub fn scenes_root() -> PathBuf {
    match std::env::var_os("MOLGFX_SCENES") {
        Some(root) => PathBuf::from(root),
        None => repo_root().join(SCENES),
    }
}

/// Resolves one scene, or nothing when the caller declared none.
///
/// A path carrying a directory is taken as written; a bare name is looked up
/// inside [`scenes_root`], gaining the `.cif` extension it is written without.
#[must_use]
pub fn scene(requested: Option<&Path>) -> Option<PathBuf> {
    let requested = requested?;
    if requested.components().count() > 1 {
        return Some(requested.to_path_buf());
    }
    Some(scenes_root().join(named(requested)))
}

/// A bare name gets the `.cif` extension; a name that already carries one is
/// taken as written, so a declared `.pdb` fixture is never renamed.
fn named(requested: &Path) -> PathBuf {
    match requested.extension() {
        Some(_) => requested.to_path_buf(),
        None => requested.with_extension("cif"),
    }
}

#[cfg(test)]
#[path = "fixtures_tests.rs"]
mod tests;
