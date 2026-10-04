//! CLI configuration and manifest validation before engine initialization.
use super::external::Programs;
use molgfx_bench::gallery::{Catalog, Fixture, Result};
use std::{io, path::PathBuf};

pub(super) struct Config {
    pub catalog: Catalog,
    pub cache: PathBuf,
    pub output: PathBuf,
    pub case: Vec<String>,
    pub recipes: Vec<String>,
    pub inspect: bool,
    pub programs: Programs,
    pub references: Option<PathBuf>,
    pub bless: bool,
}

impl Config {
    pub(super) fn read() -> Result<Self> {
        let mut manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("parity/corpus.json");
        let (mut cache, mut output) = (None, None);
        let mut case = Vec::new();
        let (mut extent, mut warmup, mut outputs) = (None, None, None);
        let mut recipes = Vec::new();
        let mut inspect = false;
        let mut bless = false;
        let mut references = None;
        let mut programs = Programs {
            node: "node".into(),
            molstar_root: "../molstar".into(),
            pymol: "pymol".into(),
            ffmpeg: None,
        };
        let mut args = std::env::args().skip(1);
        while let Some(flag) = args.next() {
            if flag == "--inspect" {
                inspect = true;
                continue;
            }
            if flag == "--bless" {
                bless = true;
                continue;
            }
            let value = args
                .next()
                .ok_or_else(|| io::Error::other(format!("missing value for {flag}")))?;
            match flag.as_str() {
                "--manifest" => manifest = value.into(),
                "--cache" => cache = Some(PathBuf::from(value)),
                "--output" => output = Some(PathBuf::from(value)),
                "--case" => case.push(value),
                "--references" => references = Some(PathBuf::from(value)),
                "--ffmpeg" => programs.ffmpeg = Some(value),
                "--recipe" => recipes.push(value),
                "--warmup" => warmup = Some(value.parse::<usize>()?),
                "--outputs" => outputs = Some(value.parse::<usize>()?),
                "--size" => {
                    let (w, h) = value
                        .split_once('x')
                        .ok_or_else(|| io::Error::other("size requires WxH"))?;
                    extent = Some([w.parse::<u32>()?, h.parse::<u32>()?]);
                }
                "--node" => programs.node = value,
                "--molstar-root" => programs.molstar_root = value,
                "--pymol" => programs.pymol = value,
                _ => return Err(io::Error::other(format!("unknown parity option {flag}")).into()),
            }
        }
        let cache = std::fs::canonicalize(
            cache.ok_or_else(|| io::Error::other("--cache is required; populate corpus first"))?,
        )?;
        let output = output.ok_or_else(|| io::Error::other("--output is required"))?;
        std::fs::create_dir_all(&output)?;
        let output = std::fs::canonicalize(output)?;
        let mut catalog: Catalog = serde_json::from_slice(&std::fs::read(manifest)?)?;
        if let Some(value) = extent {
            catalog.extent = value;
        }
        if let Some(value) = warmup {
            catalog.warmup_outputs = value;
        }
        if let Some(value) = outputs {
            catalog.measured_outputs = value;
        }
        if recipes.is_empty() {
            recipes.clone_from(&catalog.recipes);
        }
        let config = Self {
            catalog,
            cache,
            output,
            case,
            recipes,
            inspect,
            programs,
            references,
            bless,
        };
        config.verify()?;
        Ok(config)
    }

    fn verify(&self) -> Result<()> {
        if self.catalog.measured_outputs == 0 || self.catalog.extent.contains(&0) {
            return Err(io::Error::other("outputs/extent must be positive").into());
        }
        for recipe in &self.recipes {
            if !self.catalog.recipes.contains(recipe) {
                return Err(io::Error::other(format!("unknown recipe {recipe}")).into());
            }
        }
        for id in &self.case {
            if !self.catalog.fixtures.iter().any(|f| &f.id == id) {
                return Err(io::Error::other(format!("unknown corpus case {id}")).into());
            }
        }
        if self.bless && self.references.is_none() {
            return Err(io::Error::other("--bless requires --references").into());
        }
        // Missing or altered bytes fail before any engine is initialized.
        for fixture in self.selected() {
            fixture.verify(&self.cache)?;
        }
        Ok(())
    }

    /// Cases to run in manifest order; no `--case` selects every case.
    pub(super) fn selected(&self) -> impl Iterator<Item = &Fixture> {
        self.catalog
            .fixtures
            .iter()
            .filter(|f| self.case.is_empty() || self.case.contains(&f.id))
    }
}
