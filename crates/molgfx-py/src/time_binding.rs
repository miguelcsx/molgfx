//! Typed physical-time parsing shared by the Python adapters.

use molgfx::time::{SimTime, TimeUnit};
use pyo3::prelude::*;
use pyo3::{PyResult, exceptions::PyValueError};

/// Parses a unit symbol; an unknown one is a `ValueError`, never seconds.
pub(crate) fn unit_from_symbol(symbol: &str) -> PyResult<TimeUnit> {
    TimeUnit::from_symbol(symbol)
        .ok_or_else(|| PyValueError::new_err(format!("unknown time unit `{symbol}`")))
}

/// Reads a `molgfx.time.TimeUnit` (a `str` subclass) from Python.
pub(crate) fn parse_unit(value: &Bound<'_, PyAny>) -> PyResult<TimeUnit> {
    let symbol = value
        .extract::<String>()
        .map_err(|_| PyValueError::new_err("a time unit must be a molgfx.time.TimeUnit member"))?;
    unit_from_symbol(&symbol)
}

/// Converts frame times given in `unit` to canonical seconds, once.
///
/// A time that is not finite (molframe writes NaN for a frame without one)
/// names its frame.
pub(crate) fn frame_times(values: &[f64], unit: TimeUnit) -> PyResult<Vec<SimTime>> {
    values
        .iter()
        .enumerate()
        .map(|(frame, &value)| {
            if !value.is_finite() {
                return Err(PyValueError::new_err(format!(
                    "frame {frame} of the trajectory has no finite time ({value}); \
                     playback needs a recorded, finite time for every frame"
                )));
            }
            SimTime::from_unit(value, unit)
                .map_err(|error| PyValueError::new_err(format!("frame {frame}: {error}")))
        })
        .collect()
}

/// Parses an authored trajectory clock: a unit symbol and a step in that unit.
///
/// An unknown symbol is an error, and a step without a unit is ambiguous.
pub(crate) fn trajectory_clock(
    step: Option<f64>,
    unit: Option<&str>,
) -> PyResult<(Option<SimTime>, Option<TimeUnit>)> {
    let unit = unit.map(unit_from_symbol).transpose()?;
    let step = match (step, unit) {
        (Some(value), Some(unit)) => Some(
            SimTime::from_unit(value, unit)
                .map_err(|error| PyValueError::new_err(error.to_string()))?,
        ),
        (Some(_), None) => return Err(PyValueError::new_err("time_step requires a time_unit")),
        (None, _) => None,
    };
    Ok((step, unit))
}
