//! One manifest and one summarizer across independently identified recipes.

mod config;
mod external;
mod golden;
mod metadata;
mod native;
mod runner;
mod script;
mod sheet;
mod telemetry;

pub(super) use runner::run;
