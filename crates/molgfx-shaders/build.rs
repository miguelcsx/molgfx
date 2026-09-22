//! Composes and validates the WGSL library at build time.
//!
//! Shared routines live once under `src/wgsl/include/` and are pulled into
//! composed shaders by an `//!include "path"` directive — included, never
//! copied. Every composed unit is parsed and validated with naga here, so
//! shader errors fail the build rather than surfacing on the first frame.
//! Composed sources are plain standard WGSL.

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
        if let Err(diagnostic) = validate(name, &composed) {
            diagnostics.push(diagnostic);
        }
        fs::write(out_dir.join(name), &composed)?;

        // A unit that carries the specialization marker also publishes a
        // sibling whose interpreter is replaced by generated straight-line
        // code. The placeholder body is a valid stand-in for that code, so a
        // unit whose surrounding declarations were broken fails here rather
        // than on the frame that first needs a specialized style.
        if let Some(specialized) = specialize(&composed) {
            let specialized_name = specialized_name(name);
            if let Err(diagnostic) = validate(&specialized_name, &specialized) {
                diagnostics.push(diagnostic);
            }
            fs::write(out_dir.join(specialized_name), specialized)?;
        }
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

/// The marker a unit carries to request a specialized sibling.
const SPECIALIZATION_MARKER: &str = "// {{visual_program}}";

/// The stand-in body used to validate a specialized sibling.
///
/// It is a real `visual_resolve` with the same interface, so validation checks
/// everything the generated body will rely on — the gate, the register file,
/// the shared ladder, and every declaration the surrounding unit supplies —
/// without depending on any particular program.
const PLACEHOLDER_RESOLVE: &str = "\
fn visual_resolve(
    inputs: VisualEvaluationInputs,
    fallback: VisualFragmentResult,
) -> VisualFragmentResult {
    if !VISUAL_FRAGMENT_ENABLED || visual_config.counts.z == 0u {
        return fallback;
    }
    var registers: array<vec4f, 64>;
    registers[0] = visual_input(0u, inputs);
    return visual_resolve_registers(&registers, fallback);
}";

/// Builds a unit's specialized sibling, when the unit asks for one.
///
/// The marker is replaced by a placeholder body and the interpreter — the
/// included unit that defines the non-specialized `visual_resolve` — is
/// dropped, leaving exactly one definition of the entry point. A unit that
/// carries the marker without including the interpreter still gets a sibling,
/// but keeps its own definition alongside the placeholder; naga then reports
/// the duplicate, which is the correct outcome for a malformed unit.
fn specialize(composed: &str) -> Option<String> {
    if !composed.contains(SPECIALIZATION_MARKER) {
        return None;
    }
    let mut specialized = composed.replace(SPECIALIZATION_MARKER, PLACEHOLDER_RESOLVE);
    specialized = drop_include(&specialized, "visual/interpreter.wgsl");
    Some(specialized)
}

/// Removes one composed include, including the begin/end comments `compose`
/// writes around it.
///
/// The annotation carries the include directive's own relative path, so the
/// match is on the file name rather than on a reconstructed path.
fn drop_include(source: &str, include: &str) -> String {
    let mut out = String::with_capacity(source.len());
    let mut inside = false;
    for line in source.lines() {
        let annotation = line.trim();
        if annotation.starts_with("// -- begin include:") && annotation.contains(include) {
            inside = true;
        } else if annotation.starts_with("// -- end include:") && annotation.contains(include) {
            inside = false;
        } else if !inside {
            out.push_str(line);
            out.push('\n');
        }
    }
    out
}

/// The composed-file name of a unit's specialized sibling.
fn specialized_name(name: &str) -> String {
    match name.strip_suffix(".wgsl") {
        Some(stem) => format!("{stem}.specialized.wgsl"),
        None => format!("{name}.specialized.wgsl"),
    }
}

/// Validates one composed unit and returns diagnostics that fail the build.
fn validate(name: &str, source: &str) -> Result<(), String> {
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
        naga::valid::Capabilities::CLIP_DISTANCES | naga::valid::Capabilities::RAY_QUERY,
    );
    validator.validate(&module).map_err(|error| {
        format!(
            "{name}: WGSL validation error:\n{}",
            error.emit_to_string(source)
        )
    })?;
    Ok(())
}
