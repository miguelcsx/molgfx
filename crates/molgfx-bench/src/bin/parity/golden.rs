//! Golden-image comparison and blessing of the converged native image.
use super::sheet;
use molgfx_bench::gallery::Result;
use molgfx_bench::image_compare::{delta_e76, diff_png, ssim_luma};
use serde_json::{Value, json};
use std::{io, path::Path};

const MIN_SSIM: f64 = 0.985;
const MAX_MEAN_DELTA_E: f64 = 1.0;
const MAX_P99_DELTA_E: f64 = 8.0;
const ADAPTER_FILE: &str = "adapter.json";
/// The recipe whose image is the reference.
pub(super) const REFERENCE_RECIPE: &str = "molgfx-converged";

fn adapter() -> Value {
    let info = molgfx::system_info();
    let chosen = info
        .adapters
        .iter()
        .find(|a| a.renderer_core_compatible)
        .or(info.adapters.first());
    chosen.map_or(
        Value::Null,
        |a| json!({"name":a.name,"backend":a.backend,"device_type":a.device_type}),
    )
}

fn bless(references: &Path, id: &str, image: &Path) -> Result<Value> {
    std::fs::create_dir_all(references)?;
    std::fs::copy(image, references.join(format!("{id}.png")))?;
    std::fs::write(
        references.join(ADAPTER_FILE),
        serde_json::to_vec_pretty(&adapter())?,
    )?;
    Ok(json!({"status":"blessed"}))
}

/// Compares (or blesses) `directory/<recipe>/image.png` against `references/<id>.png`.
///
/// A mismatch on an adapter other than the reference one is advisory (`W0006`).
pub(super) fn check(
    references: &Path,
    id: &str,
    directory: &Path,
    bless_now: bool,
) -> Result<Value> {
    let image = directory.join(REFERENCE_RECIPE).join("image.png");
    if bless_now {
        return bless(references, id, &image);
    }
    let reference = references.join(format!("{id}.png"));
    if !reference.exists() {
        return Err(io::Error::other(format!("no reference image {}", reference.display())).into());
    }
    let (current, golden) = (sheet::decode(&image)?, sheet::decode(&reference)?);
    if (current.width, current.height) != (golden.width, golden.height) {
        return Err(io::Error::other(format!(
            "{id}: image is {}x{}, reference is {}x{}; render with the blessed --size",
            current.width, current.height, golden.width, golden.height
        ))
        .into());
    }
    let (width, height) = (
        u32::try_from(current.width)?,
        u32::try_from(current.height)?,
    );
    let ssim = ssim_luma(&current.rgba, &golden.rgba, width, height);
    let delta = delta_e76(&current.rgba, &golden.rgba, width, height);
    let passed = ssim >= MIN_SSIM && delta.mean <= MAX_MEAN_DELTA_E && delta.p99 <= MAX_P99_DELTA_E;
    let reference_adapter = std::fs::read(references.join(ADAPTER_FILE))
        .ok()
        .and_then(|bytes| serde_json::from_slice::<Value>(&bytes).ok());
    let advisory = reference_adapter
        .as_ref()
        .is_some_and(|a| a["name"] != adapter()["name"]);
    if !passed {
        std::fs::write(
            directory.join("diff.png"),
            diff_png(&current.rgba, &golden.rgba, width, height)?,
        )?;
    }
    if !passed && advisory {
        eprintln!("W0006 advisory: non-reference adapter; {id} differs from its reference");
    }
    Ok(
        json!({"status":if passed {"passed"} else if advisory {"advisory"} else {"failed"},
        "ssim":ssim,"mean_delta_e":delta.mean,"p99_delta_e":delta.p99}),
    )
}
