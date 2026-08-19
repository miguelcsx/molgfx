//! Composes, validates, and cross-compiles the WGSL library at build time.
//!
//! Shared routines live once under `src/wgsl/include/` and are pulled into
//! composed shaders by an `//!include "path"` directive — included, never
//! copied. Every composed unit is validated and emitted as SPIR-V with naga
//! here, so either frontend or backend errors fail the build rather than
//! surfacing on the first frame. Composed sources are plain standard WGSL.

use std::collections::HashSet;
use std::error::Error;
use std::fmt::Write as _;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

fn main() -> Result<(), Box<dyn Error>> {
    let wgsl_dir = PathBuf::from("src/wgsl");
    let out_dir = PathBuf::from(std::env::var("OUT_DIR")?);
    track_sources(&wgsl_dir)?;

    let mut entries = Vec::new();
    collect_entries(&wgsl_dir, &wgsl_dir.join("include"), &mut entries)?;

    // Every unit is validated before the first failure is reported, so one
    // build surfaces every broken shader rather than only the first.
    let mut diagnostics = Vec::new();
    for path in entries {
        let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
            return Err(
                io::Error::new(io::ErrorKind::InvalidData, "WGSL filename is not UTF-8").into(),
            );
        };
        let mut included = HashSet::new();
        let composed = compose(&path, &wgsl_dir, &mut included)?;
        match validate_and_compile_spirv(name, &composed) {
            Ok(spirv) => {
                let spirv_path = out_dir.join(Path::new(name).with_extension("spv"));
                fs::write(spirv_path, spirv_bytes(&spirv))?;
            }
            Err(diagnostic) => diagnostics.push(diagnostic),
        }
        fs::write(out_dir.join(name), composed)?;
    }
    if !diagnostics.is_empty() {
        return Err(io::Error::other(diagnostics.join("\n")).into());
    }
    Ok(())
}

fn collect_entries(
    directory: &Path,
    include_directory: &Path,
    entries: &mut Vec<PathBuf>,
) -> Result<(), io::Error> {
    for entry in fs::read_dir(directory)? {
        let path = entry?.path();
        if path == include_directory {
            continue;
        }
        if path.is_dir() {
            collect_entries(&path, include_directory, entries)?;
        } else if path.extension().and_then(|extension| extension.to_str()) == Some("wgsl") {
            entries.push(path);
        }
    }
    entries.sort();
    Ok(())
}

fn track_sources(directory: &Path) -> Result<(), io::Error> {
    // Cargo must also watch the directory entry itself: tracking only the
    // current files misses a newly added shader and can leave `include_str!`
    // pointing at an output the otherwise-fresh build script never composed.
    println!("cargo:rerun-if-changed={}", directory.display());
    for entry in fs::read_dir(directory)? {
        let path = entry?.path();
        if path.is_dir() {
            track_sources(&path)?;
        } else if path.extension().and_then(|extension| extension.to_str()) == Some("wgsl") {
            println!("cargo:rerun-if-changed={}", path.display());
        }
    }
    Ok(())
}

/// Recursively inlines `//!include "relative/path.wgsl"` directives. Each
/// include lands once per composed unit; cycles are refused.
fn compose(path: &Path, root: &Path, included: &mut HashSet<PathBuf>) -> Result<String, io::Error> {
    let source = fs::read_to_string(path)?;
    let mut out = String::with_capacity(source.len());
    for line in source.lines() {
        let trimmed = line.trim();
        if let Some(rest) = trimmed.strip_prefix("//!include ") {
            let rel = rest.trim().trim_matches('"');
            let target = root.join(rel);
            let canonical = target.canonicalize()?;
            if included.insert(canonical.clone()) {
                let _ = writeln!(out, "// -- begin include: {rel}");
                out.push_str(&compose(&canonical, root, included)?);
                let _ = writeln!(out, "// -- end include: {rel}");
            }
        } else {
            out.push_str(line);
            out.push('\n');
        }
    }
    Ok(out)
}

/// Validates one composed unit and returns diagnostics that fail the build.
fn validate_and_compile_spirv(name: &str, source: &str) -> Result<Vec<u32>, String> {
    let module = match naga::front::wgsl::parse_str(source) {
        Ok(module) => module,
        Err(e) => {
            return Err(format!(
                "{name}: WGSL parse error:\n{}",
                e.emit_to_string(source)
            ));
        }
    };
    let mut validator = naga::valid::Validator::new(
        naga::valid::ValidationFlags::all(),
        naga::valid::Capabilities::CLIP_DISTANCES,
    );
    let info = validator.validate(&module).map_err(|error| {
        format!(
            "{name}: WGSL validation error:\n{}",
            error.emit_to_string(source)
        )
    })?;
    let mut pipeline_constants = naga::back::PipelineConstants::default();
    for (_, constant) in module.overrides.iter() {
        if constant.init.is_none() {
            let Some(override_name) = &constant.name else {
                return Err(format!(
                    "{name}: required pipeline override has neither a name nor a default"
                ));
            };
            pipeline_constants.insert(override_name.clone(), 0.0);
        }
    }
    let (specialized, specialized_info) = naga::back::pipeline_constants::process_overrides(
        &module,
        &info,
        None,
        &pipeline_constants,
    )
    .map_err(|error| format!("{name}: override specialization error: {error}"))?;
    naga::back::spv::write_vec(
        &specialized,
        &specialized_info,
        &naga::back::spv::Options::default(),
        None,
    )
    .map_err(|error| format!("{name}: SPIR-V emission error: {error}"))
}

fn spirv_bytes(words: &[u32]) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(std::mem::size_of_val(words));
    for word in words {
        bytes.extend_from_slice(&word.to_le_bytes());
    }
    bytes
}
