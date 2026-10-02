//! A pass built the first time a frame draws with it.
//!
//! A pass owns GPU pipelines, and a pipeline costs driver memory and compile
//! time whether or not anything is ever drawn with it. A scene that shows a
//! cartoon never needs the sphere, surface, transparency or primitive
//! pipelines, so each of those is built at its first draw and not before.

use crate::error::RenderError;
use std::sync::OnceLock;

#[derive(Debug)]
pub(crate) struct Lazy<T>(OnceLock<T>);

impl<T> Default for Lazy<T> {
    fn default() -> Self {
        Self(OnceLock::new())
    }
}

impl<T> Lazy<T> {
    /// The value if some frame has already built it.
    pub(crate) fn get(&self) -> Option<&T> {
        self.0.get()
    }

    /// The value, built by `build` on first use.
    ///
    /// # Errors
    /// Returns the builder's error; the value stays unbuilt and a later frame
    /// tries again.
    pub(crate) fn get_or_build(
        &self,
        build: impl FnOnce() -> Result<T, RenderError>,
    ) -> Result<&T, RenderError> {
        if let Some(value) = self.0.get() {
            return Ok(value);
        }
        let built = build()?;
        Ok(self.0.get_or_init(|| built))
    }
}

impl<T> Lazy<T> {
    /// The value built on first use, keeping a failure in `failure`.
    ///
    /// For a record function that already holds the encoder: it passes the
    /// context's fields separately, so building does not borrow the context.
    pub(crate) fn build_in<D: molgfx_gpu::Device>(
        &self,
        env: &crate::graph::PassEnv<'_, D>,
        failure: &mut Option<RenderError>,
        make: impl FnOnce(&crate::graph::PassEnv<'_, D>) -> Result<T, RenderError>,
    ) -> Option<&T> {
        match self.get_or_build(|| make(env)) {
            Ok(value) => Some(value),
            Err(error) => {
                failure.get_or_insert(error);
                None
            }
        }
    }
}

#[cfg(test)]
#[path = "lazy_tests.rs"]
mod tests;
