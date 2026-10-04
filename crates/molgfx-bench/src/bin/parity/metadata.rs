//! Metadata inspection precedes all image production for each fixture.
use super::{config::Config, external};
use molgfx_bench::gallery::{Fixture, Result};
use serde_json::{Value, json};
use std::{collections::BTreeMap, io, path::Path};

pub(super) fn engine(recipe: &str) -> Result<&str> {
    recipe
        .split('-')
        .next()
        .ok_or_else(|| io::Error::other("invalid recipe").into())
}

pub(super) fn inspect(config: &Config, fixture: &Fixture, directory: &Path) -> Result<Value> {
    let mut metadata = BTreeMap::new();
    for recipe in &config.recipes {
        let engine = engine(recipe)?;
        if metadata.contains_key(engine) {
            continue;
        }
        let result = if engine == "molgfx" {
            if fixture.details.contains_key("segmentations") {
                molgfx_bench::gallery::segmentation_scene(fixture, &config.cache)
                    .map(|(_, metadata)| metadata)
            } else if fixture.format == "mrc" {
                molgfx_bench::gallery::density(fixture, &config.cache).map(|(_, metadata)| metadata)
            } else {
                molgfx_bench::gallery::structure(fixture, &config.cache)
                    .map(|(_, metadata)| metadata)
            }
        } else {
            let path = directory.join(format!("{engine}-inspect"));
            std::fs::create_dir_all(&path)?;
            external::invoke(
                &config.catalog,
                fixture,
                &config.cache,
                &path,
                recipe,
                true,
                &config.programs,
            )
            .map(|r| r["metadata"].clone())
        };
        metadata.insert(
            engine.to_owned(),
            match result {
                Ok(m) => m,
                Err(e) => json!({"error":e.to_string()}),
            },
        );
    }
    let baseline = metadata.iter().find(|(_, v)| v.get("error").is_none());
    let mut differences = Vec::new();
    if let Some((baseline_engine, base)) = baseline {
        for (engine, m) in &metadata {
            for field in [
                "atoms",
                "residues",
                "bonds",
                "aromatic_bonds",
                "secondary_structure",
            ] {
                if m[field] != base[field] {
                    differences.push(json!({"engine":engine,"field":field,"baseline_engine":baseline_engine,"baseline":base[field],"observed":m[field]}));
                }
            }
        }
    }
    Ok(json!({"metadata":metadata,"differences":differences,
        "provenance_comparison":"Provider/file/inference provenance reported explicitly; opaque external provenance is not certified equivalent",
        "image_parity_certified":false}))
}
