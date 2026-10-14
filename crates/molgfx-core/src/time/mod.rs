//! Typed clocks for simulation time and presentation time.
//!
//! A scene runs on two clocks that must never be confused. [`PlaybackTime`] is
//! the presentation clock that a viewer or a frame writer advances; frame `i` of
//! a sequence at `fps` is exactly `i / fps` on it. [`SimTime`] is the physical
//! time a simulation recorded, which can span femtoseconds to milliseconds within
//! one scene. Each timeline track maps playback time onto its own simulation
//! time with its own rate, so a bond vibration and a domain opening each play at
//! their natural timescale on one playback clock.
//!
//! Both clocks store canonical SI seconds as `f64`. A double resolves a
//! one-femtosecond step next to a one-millisecond timestamp, so the unit never
//! changes the arithmetic. The unit lives only at the boundary: [`SimTime::from_unit`]
//! accepts a value recorded in picoseconds, and [`SimTime::to_unit`] or the
//! [`Display`](std::fmt::Display) implementation reports one back. Narrowing to
//! `f32` happens only where a GPU uniform requires it.

mod playback_time;
mod sim_time;
mod track_clock;
mod unit;

pub use playback_time::PlaybackTime;
pub use sim_time::SimTime;
pub use track_clock::TrackClock;
pub use unit::TimeUnit;

/// Narrows a canonical `f64` quantity to the `f32` a GPU uniform or a CPU blend
/// consumes. `None` means the value has no finite `f32` representation.
#[must_use]
pub fn narrow_to_f32(value: f64) -> Option<f32> {
    num_traits::cast::<f64, f32>(value).filter(|narrowed| narrowed.is_finite())
}
