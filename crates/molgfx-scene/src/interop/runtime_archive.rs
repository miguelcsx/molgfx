//! Scientific assets travel separately from molecular inputs and share restored frame owners.

mod capture;
mod control;
mod frames;
mod restore;
mod types;

pub use capture::RuntimeCapture;
pub use types::SceneArchive;

#[cfg(test)]
mod tests;
