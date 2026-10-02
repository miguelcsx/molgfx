//! Execute every selected fixture, retaining failures and pre-image inspection.
use super::{
    catalog::{Fixture, Result},
    config::Config,
    external, golden, metadata, native, script, sheet, telemetry,
};
use serde_json::{Value, json};
use std::{io, path::Path};

pub(crate) fn run() -> Result<()> {
    let config = Config::read()?;
    let mut report = Vec::new();
    let mut failures = 0;
    // Native recipes run first so a fitted camera and native frames exist before external adapters.
    let mut order = config.recipes.clone();
    order.sort_by_key(|recipe| recipe.split('-').next() != Some("molgfx"));
    for original in config.selected() {
        let directory = config.output.join(&original.id);
        std::fs::create_dir_all(&directory)?;
        let mut fixture = original.clone();
        if let Some(fit) = fixture.script.as_ref().and_then(|s| s.fit.as_ref()) {
            match script::fit(&fixture, &config.cache, fit) {
                Ok(camera) => {
                    save(
                        &directory.join("camera.json"),
                        &serde_json::to_value(&camera)?,
                    )?;
                    fixture.camera = camera;
                }
                Err(error) => {
                    failures += 1;
                    let entry =
                        json!({"status":"failed","fixture":fixture.id,"error":error.to_string()});
                    println!("{entry}");
                    save(&directory.join("report.json"), &entry)?;
                    report.push(entry);
                    continue;
                }
            }
        }
        let fixture = &fixture;
        let comparison = metadata::inspect(&config, fixture, &directory)?;
        save(&directory.join("metadata.json"), &comparison)?;
        for recipe in &order {
            let path = directory.join(recipe);
            std::fs::create_dir_all(&path)?;
            let mut entry = match execute(&config, fixture, recipe, &path, &comparison) {
                Ok(mut value) => {
                    value["status"] = json!("executed");
                    value
                }
                Err(error) => {
                    failures += 1;
                    json!({"status":"failed","error":error.to_string()})
                }
            };
            entry["fixture"] = serde_json::to_value(fixture)?;
            entry["recipe"] = json!(recipe);
            entry["metadata_comparison"] = comparison.clone();
            entry["cross_recipe_speedup"] = Value::Null;
            save(&path.join("report.json"), &entry)?;
            println!(
                "{}",
                json!({"fixture":fixture.id,"recipe":recipe,"status":entry["status"]})
            );
            report.push(entry);
        }
        if !config.inspect {
            let missing = sheet::write(&directory, &config.recipes, config.catalog.extent)?;
            if !missing.is_empty() {
                failures += missing.len();
                println!("{}", json!({"fixture":fixture.id,"sheet_missing":missing}));
            }
            if let Some(references) = &config.references {
                let verdict = match golden::check(references, &fixture.id, &directory, config.bless)
                {
                    Ok(verdict) => verdict,
                    Err(error) => json!({"status":"failed","error":error.to_string()}),
                };
                if verdict["status"] == "failed" {
                    failures += 1;
                }
                println!("{}", json!({"fixture":fixture.id,"golden":verdict}));
                save(&directory.join("golden.json"), &verdict)?;
            }
        }
    }
    save(
        &config.output.join("report.json"),
        &json!({"catalog":config.catalog,"filtered_case":config.case,
        "recipes":config.recipes,"inspection_only":config.inspect,"failures":failures,"results":report,
        "parity_certified":false,"reason":"Execution evidence is not physical equivalence; recipes, light rigs, transport, SS and provenance differ. No speedup claims."}),
    )?;
    if failures > 0 {
        return Err(io::Error::other(format!(
            "{failures} parity recipes failed; see {}",
            config.output.display()
        ))
        .into());
    }
    Ok(())
}

fn execute(
    config: &Config,
    fixture: &Fixture,
    recipe: &str,
    path: &Path,
    comparison: &Value,
) -> Result<Value> {
    let engine = metadata::engine(recipe)?;
    let metadata = &comparison["metadata"][engine];
    if let Some(error) = metadata.get("error") {
        return Err(io::Error::other(error.to_string()).into());
    }
    if config.inspect {
        return Ok(json!({"metadata":metadata,"inspection_only":true}));
    }
    if engine == "molgfx" {
        native::render(
            &config.catalog,
            fixture,
            &config.cache,
            path,
            recipe,
            config.programs.ffmpeg.as_deref(),
        )
    } else {
        let mut report = external::invoke(
            &config.catalog,
            fixture,
            &config.cache,
            path,
            recipe,
            false,
            &config.programs,
        )?;
        telemetry::normalize_external(&mut report)?;
        Ok(report)
    }
}

fn save(path: &Path, value: &Value) -> Result<()> {
    std::fs::write(path, serde_json::to_vec_pretty(value)?)?;
    Ok(())
}
