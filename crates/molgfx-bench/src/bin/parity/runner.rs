//! Execute every selected fixture, retaining failures and pre-image inspection.
use super::{
    catalog::{Fixture, Result},
    config::Config,
    external, metadata, native, telemetry,
};
use serde_json::{Value, json};
use std::{io, path::Path};

pub(crate) fn run() -> Result<()> {
    let config = Config::read()?;
    let mut report = Vec::new();
    let mut failures = 0;
    for fixture in config
        .catalog
        .fixtures
        .iter()
        .filter(|f| config.case.as_ref().is_none_or(|id| &f.id == id))
    {
        let directory = config.output.join(&fixture.id);
        std::fs::create_dir_all(&directory)?;
        let comparison = metadata::inspect(&config, fixture, &directory)?;
        save(&directory.join("metadata.json"), &comparison)?;
        for recipe in &config.recipes {
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
        native::render(&config.catalog, fixture, &config.cache, path, recipe)
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
