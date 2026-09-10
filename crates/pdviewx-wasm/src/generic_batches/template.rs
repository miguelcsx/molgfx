//! Shared analytic sphere/capsule templates.

use super::values::{core_error, js_value, ordered_rows};
use std::sync::Arc;
use wasm_bindgen::prelude::*;

/// Shared homogeneous sphere/capsule geometry retained once by all instances.
#[wasm_bindgen]
#[derive(Clone, Debug)]
pub struct WebAnalyticTemplate {
    pub(super) inner: Arc<pdviewx::AnalyticTemplate>,
}

#[wasm_bindgen]
impl WebAnalyticTemplate {
    /// Builds from flat sphere `[x,y,z,r]` and capsule
    /// `[ax,ay,az,bx,by,bz,r]` arrays.
    ///
    /// # Errors
    ///
    /// Returns a JavaScript error for incomplete or invalid analytic records.
    #[wasm_bindgen(constructor)]
    #[allow(clippy::needless_pass_by_value)]
    pub fn new(
        namespace: u64,
        spheres: Vec<f32>,
        capsules: Vec<f32>,
    ) -> Result<WebAnalyticTemplate, JsValue> {
        if !spheres.len().is_multiple_of(4) || !capsules.len().is_multiple_of(7) {
            return Err(js_value(
                "sphere and capsule arrays must contain complete flat records",
            ));
        }
        let spheres = spheres
            .chunks_exact(4)
            .map(|row| pdviewx::AnalyticSphere {
                center: [row[0], row[1], row[2]],
                radius: row[3],
            })
            .collect::<Vec<_>>();
        let mut packed_capsules = Vec::with_capacity(capsules.len() / 7);
        for row in capsules.chunks_exact(7) {
            packed_capsules.push(
                pdviewx::AnalyticCapsule::new(
                    pdviewx::Vec3::new(row[0], row[1], row[2]),
                    pdviewx::Vec3::new(row[3], row[4], row[5]),
                    row[6],
                )
                .map_err(core_error)?,
            );
        }
        let rows = ordered_rows(
            namespace,
            spheres.len().saturating_add(packed_capsules.len()),
        )?;
        let inner =
            pdviewx::AnalyticTemplate::new(Arc::from(spheres), Arc::from(packed_capsules), rows)
                .map_err(core_error)?;
        Ok(Self {
            inner: Arc::new(inner),
        })
    }

    /// Number of homogeneous analytic parts shared by every instance.
    #[must_use]
    #[wasm_bindgen(getter, js_name = partCount)]
    pub fn part_count(&self) -> usize {
        self.inner.part_count()
    }
}
