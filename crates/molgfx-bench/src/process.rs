//! OS process measurements around already-built consumer binaries.

use serde::Serialize;
use std::fs::{self, File};
use std::io;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

#[cfg(test)]
#[path = "process_tests.rs"]
mod tests;

/// Peak process RSS, independent of heap and logical GPU byte counters.
#[derive(Debug, Serialize)]
pub struct ProcessMeasurement {
    /// Darwin's `ru_maxrss`, whose unit is bytes, without Linux's KiB conversion.
    pub peak_rss_bytes: u64,
    /// Preserved unmodified OS measurement output.
    pub rss_raw: PathBuf,
    /// Consumer output, kept separate from OS diagnostics.
    pub stdout: PathBuf,
    /// Consumer error output.
    pub stderr: PathBuf,
}

/// Runs one child binary under Darwin's OS resource meter.
/// The peak covers the entire child, including fixtures/backend/driver mappings;
/// it never covers compilation or this runner's own memory.
///
/// # Errors
/// Fails outside Darwin, on child failure, or missing/malformed RSS output.
pub fn measure_process(
    executable: &Path,
    arguments: &[String],
    output: &Path,
) -> Result<ProcessMeasurement, io::Error> {
    if !cfg!(target_os = "macos") {
        return Err(io::Error::other(
            "isolated RSS currently requires Darwin /usr/bin/time -l",
        ));
    }
    fs::create_dir_all(output)?;
    let measurement = ProcessMeasurement {
        peak_rss_bytes: 0,
        rss_raw: output.join("rss.raw.txt"),
        stdout: output.join("stdout.jsonl"),
        stderr: output.join("stderr.txt"),
    };
    // Refuse accidental replacement of evidence from a previous trial.
    let stdout = File::create_new(&measurement.stdout)?;
    let stderr = File::create_new(&measurement.stderr)?;
    File::create_new(&measurement.rss_raw)?;
    let status = Command::new("/usr/bin/time")
        .arg("-l")
        .arg("-o")
        .arg(&measurement.rss_raw)
        .arg(executable)
        .args(arguments)
        .stdin(Stdio::null())
        .stdout(stdout)
        .stderr(stderr)
        .status()?;
    if !status.success() {
        return Err(io::Error::other(format!(
            "isolated consumer exited {status}; diagnostics: {}",
            measurement.stderr.display()
        )));
    }
    let raw = fs::read_to_string(&measurement.rss_raw)?;
    Ok(ProcessMeasurement {
        peak_rss_bytes: darwin_peak_rss(&raw)?,
        ..measurement
    })
}

fn darwin_peak_rss(raw: &str) -> Result<u64, io::Error> {
    let mut result = None;
    for line in raw.lines() {
        let Some(value) = line.trim().strip_suffix("maximum resident set size") else {
            continue;
        };
        if result.is_some() {
            return Err(io::Error::other(
                "duplicate Darwin maximum resident set size",
            ));
        }
        result = Some(value.trim().parse::<u64>().map_err(io::Error::other)?);
    }
    result
        .ok_or_else(|| io::Error::other("Darwin time output is missing maximum resident set size"))
}
