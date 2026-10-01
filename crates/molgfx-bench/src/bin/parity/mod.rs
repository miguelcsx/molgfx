//! One manifest and one summarizer across independently identified recipes.

mod catalog;
mod config;
mod external;
mod metadata;
mod native;
mod runner;
mod telemetry;

pub(super) use runner::run;
