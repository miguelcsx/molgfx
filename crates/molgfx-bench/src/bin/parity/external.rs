//! Process-isolated adapters using real engine libraries, not guessed CLI flags.
use molgfx_bench::gallery::{Catalog, Fixture, Result};
use serde_json::{Value, json};
use std::{env, io, path::Path, process::Command};

pub(super) struct Programs {
    pub node: String,
    pub molstar_root: String,
    pub pymol: String,
    pub ffmpeg: Option<String>,
}
pub(super) fn invoke(
    catalog: &Catalog,
    fixture: &Fixture,
    cache: &Path,
    output: &Path,
    recipe: &str,
    inspect: bool,
    programs: &Programs,
) -> Result<Value> {
    let request = output.join("request.json");
    let response = output.join("response.json");
    match std::fs::remove_file(&response) {
        Ok(()) => {}
        Err(error) if error.kind() == io::ErrorKind::NotFound => {}
        Err(error) => return Err(error.into()),
    }
    let affine = if fixture.format == "mrc" {
        Some(molgfx_bench::reader::read_density_map(&cache.join(&fixture.file))?.1)
    } else {
        None
    };
    let json = json!({"catalog":catalog,"fixture":fixture,"path":cache.join(&fixture.file),
        "output":output,"response":response,"recipe":recipe,"inspect":inspect,
        "voxel_to_world":affine});
    std::fs::write(&request, serde_json::to_vec_pretty(&json)?)?;
    let scripts = Path::new(env!("CARGO_MANIFEST_DIR")).join("parity");
    let mut command = if recipe.starts_with("molstar-") {
        let mut command = Command::new(&programs.node);
        command.arg(scripts.join("molstar.cjs"));
        command.env("MOLSTAR_ROOT", &programs.molstar_root);
        let mut module_paths = vec![Path::new(&programs.molstar_root).join("node_modules")];
        if let Some(existing) = env::var_os("NODE_PATH") {
            module_paths.extend(env::split_paths(&existing));
        }
        command.env("NODE_PATH", env::join_paths(module_paths)?);
        command
    } else {
        let mut command = Command::new(&programs.pymol);
        // Raster draw needs a real OpenGL context; -c suppresses the GUI/context.
        let mode = if recipe == "pymol-raster" && !inspect {
            "-q"
        } else {
            "-cq"
        };
        command.arg(mode).arg(scripts.join("pymol.py"));
        command
    };
    command.env("MOLGFX_PARITY_REQUEST", &request);
    let result = command.output()?;
    std::fs::write(output.join("stdout.log"), &result.stdout)?;
    std::fs::write(output.join("stderr.log"), &result.stderr)?;
    if !result.status.success() {
        return Err(io::Error::other(format!(
            "{recipe} failed: {}; {}",
            result.status,
            String::from_utf8_lossy(&result.stderr)
        ))
        .into());
    }
    // A process that exits successfully without its report has not passed.
    Ok(serde_json::from_slice(&std::fs::read(response)?)?)
}
