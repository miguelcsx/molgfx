//! Command-line options for the trajectory video.

use std::path::PathBuf;

/// Where rendered frames go.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Sink {
    /// Raw RGBA piped into ffmpeg, which encodes `video.mp4`.
    Ffmpeg,
    /// One PNG per frame under `frames/`.
    Png,
    /// Rendered and then dropped: measures the renderer alone.
    Discard,
}

/// Which H.264 encoder ffmpeg runs.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Encoder {
    /// libx264 at the medium preset with CRF 18: portable, and the default.
    X264,
    /// Apple's hardware encoder at 20 Mbit/s: the fastest and lightest on macOS.
    VideoToolbox,
}

/// Which measured output the renderer produces for each frame.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Quality {
    /// A converged exposure averaging the tier's full sample count.
    Converged,
    /// One single-sample frame, as presented while interacting.
    Interactive,
}

/// Parsed options; every path is required, every number has a default.
#[derive(Debug)]
pub(crate) struct Args {
    pub(crate) structure: PathBuf,
    pub(crate) xtc: PathBuf,
    pub(crate) psf: PathBuf,
    pub(crate) dcd: PathBuf,
    pub(crate) out: PathBuf,
    pub(crate) ffmpeg: String,
    pub(crate) seconds: f64,
    pub(crate) fps: u32,
    pub(crate) width: u32,
    pub(crate) height: u32,
    pub(crate) quality: Quality,
    pub(crate) sink: Sink,
    pub(crate) encoder: Encoder,
    pub(crate) labels: bool,
}

pub(crate) const USAGE: &str = "usage: trajectory_video --structure PDB --xtc XTC --psf PSF --dcd DCD --out DIR \
[--seconds 12] [--fps 60] [--width 1920] [--height 1080] [--quality converged|interactive] \
[--sink ffmpeg|png|discard] [--encoder x264|videotoolbox] [--ffmpeg PATH] [--no-labels]";

impl Args {
    /// Parses the options after the program name.
    ///
    /// # Errors
    /// Returns the usage text for an unknown flag, a missing value, a missing
    /// required path, or a value out of range.
    pub(crate) fn parse(mut argv: impl Iterator<Item = String>) -> Result<Self, String> {
        let mut structure = None;
        let mut xtc = None;
        let mut psf = None;
        let mut dcd = None;
        let mut out = None;
        let mut ffmpeg = String::from("ffmpeg");
        let mut seconds: f64 = 12.0;
        let mut fps = 60;
        let mut width = 1920;
        let mut height = 1080;
        let mut quality = Quality::Converged;
        let mut sink = Sink::Ffmpeg;
        let mut encoder = Encoder::X264;
        let mut labels = true;
        while let Some(flag) = argv.next() {
            if flag == "--no-labels" {
                labels = false;
                continue;
            }
            let value = argv
                .next()
                .ok_or_else(|| format!("{flag} needs a value\n{USAGE}"))?;
            match flag.as_str() {
                "--structure" => structure = Some(PathBuf::from(value)),
                "--xtc" => xtc = Some(PathBuf::from(value)),
                "--psf" => psf = Some(PathBuf::from(value)),
                "--dcd" => dcd = Some(PathBuf::from(value)),
                "--out" => out = Some(PathBuf::from(value)),
                "--ffmpeg" => ffmpeg = value,
                "--seconds" => seconds = number(&flag, &value)?,
                "--fps" => fps = number(&flag, &value)?,
                "--width" => width = number(&flag, &value)?,
                "--height" => height = number(&flag, &value)?,
                "--quality" => {
                    quality = match value.as_str() {
                        "converged" => Quality::Converged,
                        "interactive" => Quality::Interactive,
                        other => return Err(format!("unknown quality {other}\n{USAGE}")),
                    };
                }
                "--sink" => {
                    sink = match value.as_str() {
                        "ffmpeg" => Sink::Ffmpeg,
                        "png" => Sink::Png,
                        "discard" => Sink::Discard,
                        other => return Err(format!("unknown sink {other}\n{USAGE}")),
                    };
                }
                "--encoder" => {
                    encoder = match value.as_str() {
                        "x264" => Encoder::X264,
                        "videotoolbox" => Encoder::VideoToolbox,
                        other => return Err(format!("unknown encoder {other}\n{USAGE}")),
                    };
                }
                other => return Err(format!("unknown flag {other}\n{USAGE}")),
            }
        }
        let required = |name: &str, value: Option<PathBuf>| {
            value.ok_or_else(|| format!("--{name} is required\n{USAGE}"))
        };
        if !seconds.is_finite() || seconds <= 0.0 {
            return Err(format!("--seconds must be positive\n{USAGE}"));
        }
        if fps == 0 || width == 0 || height == 0 || width % 2 != 0 || height % 2 != 0 {
            return Err(format!(
                "--fps and --width/--height must be positive, and the size even (yuv420p)\n{USAGE}"
            ));
        }
        Ok(Self {
            structure: required("structure", structure)?,
            xtc: required("xtc", xtc)?,
            psf: required("psf", psf)?,
            dcd: required("dcd", dcd)?,
            out: required("out", out)?,
            ffmpeg,
            seconds,
            fps,
            width,
            height,
            quality,
            sink,
            encoder,
            labels,
        })
    }
}

fn number<T: std::str::FromStr>(flag: &str, value: &str) -> Result<T, String>
where
    T::Err: std::fmt::Display,
{
    value
        .parse()
        .map_err(|error| format!("{flag} {value}: {error}"))
}

#[cfg(test)]
#[path = "args_tests.rs"]
mod tests;
