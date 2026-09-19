//! Minimal command-line parsing for the acceptance binary.

use std::error::Error;
use std::io;
use std::path::PathBuf;

pub(crate) struct Arguments {
    pub(crate) scene: Option<String>,
    pub(crate) fixture: Option<PathBuf>,
    pub(crate) output: PathBuf,
    pub(crate) warmup: usize,
    pub(crate) frames: usize,
}

pub(crate) const MINIMUM_PERCENTILE_FRAMES: usize = 100;

pub(crate) fn arguments() -> Result<Arguments, Box<dyn Error>> {
    let values = std::env::args().skip(1).collect::<Vec<_>>();
    let mut result = Arguments {
        scene: None,
        fixture: None,
        output: PathBuf::from("target/gs-acceptance"),
        warmup: 20,
        frames: 120,
    };
    let mut index = 0;
    while index < values.len() {
        let value = values
            .get(index)
            .ok_or_else(|| io::Error::other("argument index became invalid"))?;
        let target = values.get(index + 1).ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("missing value after {value}"),
            )
        })?;
        match value.as_str() {
            "--scene" => result.scene = Some(target.clone()),
            "--fixture" => result.fixture = Some(PathBuf::from(target)),
            "--output" => result.output = PathBuf::from(target),
            "--warmup" => result.warmup = target.parse()?,
            "--frames" => result.frames = target.parse()?,
            _ => {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    format!("unknown argument {value}"),
                )
                .into());
            }
        }
        index += 2;
    }
    validate_sampling(result.warmup, result.frames)?;
    Ok(result)
}

pub(crate) fn validate_sampling(warmup: usize, frames: usize) -> Result<(), io::Error> {
    if warmup == 0 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "acceptance evidence requires at least one warmup frame",
        ));
    }
    if frames < MINIMUM_PERCENTILE_FRAMES {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!(
                "acceptance percentiles require at least {MINIMUM_PERCENTILE_FRAMES} measured frames"
            ),
        ));
    }
    Ok(())
}
