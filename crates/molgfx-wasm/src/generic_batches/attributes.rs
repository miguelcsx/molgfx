//! Native-width attribute insertion and domain introspection.

use super::{
    WebAttributeHandle, WebRowDomain,
    values::{core_error, js_value, vec3_rows},
};
use crate::browser::WebScene;
use std::sync::Arc;
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
impl WebScene {
    /// Adds one native-width scalar column to the scene-wide attribute arena.
    ///
    /// # Errors
    ///
    /// Rejects a stale domain or a column whose row count does not match it.
    #[wasm_bindgen(js_name = addScalarAttribute)]
    pub fn add_scalar_attribute(
        &mut self,
        domain: &WebRowDomain,
        name: String,
        values: Vec<f32>,
    ) -> Result<WebAttributeHandle, JsValue> {
        add_attribute(
            self,
            domain,
            name,
            molgfx::core::AttributeValues::Scalar(Arc::from(values)),
        )
    }

    /// Adds one native-width category column.
    ///
    /// # Errors
    ///
    /// Rejects a stale domain or a column whose row count does not match it.
    #[wasm_bindgen(js_name = addCategoryAttribute)]
    pub fn add_category_attribute(
        &mut self,
        domain: &WebRowDomain,
        name: String,
        values: Vec<u32>,
    ) -> Result<WebAttributeHandle, JsValue> {
        add_attribute(
            self,
            domain,
            name,
            molgfx::core::AttributeValues::Category(Arc::from(values)),
        )
    }

    /// Adds tightly packed xyz vector rows without `vec3` padding.
    ///
    /// # Errors
    ///
    /// Rejects malformed xyz storage, stale domains or mismatched row counts.
    #[wasm_bindgen(js_name = addVectorAttribute)]
    pub fn add_vector_attribute(
        &mut self,
        domain: &WebRowDomain,
        name: String,
        values: Vec<f32>,
    ) -> Result<WebAttributeHandle, JsValue> {
        let values = vec3_rows("values", values)?;
        add_attribute(
            self,
            domain,
            name,
            molgfx::core::AttributeValues::Vector(Arc::from(values)),
        )
    }

    /// Adds packed RGBA8 rows from one flat typed array.
    ///
    /// # Errors
    ///
    /// Rejects malformed RGBA storage, stale domains or mismatched row counts.
    #[wasm_bindgen(js_name = addColorAttribute)]
    #[allow(clippy::needless_pass_by_value)]
    pub fn add_color_attribute(
        &mut self,
        domain: &WebRowDomain,
        name: String,
        values: Vec<u8>,
    ) -> Result<WebAttributeHandle, JsValue> {
        if !values.len().is_multiple_of(4) {
            return Err(js_value("color values must contain flat RGBA rows"));
        }
        let values = values
            .chunks_exact(4)
            .map(|row| molgfx::math::Rgba8::new(row[0], row[1], row[2], row[3]))
            .collect::<Vec<_>>();
        add_attribute(
            self,
            domain,
            name,
            molgfx::core::AttributeValues::Color(Arc::from(values)),
        )
    }

    /// Changes generic batch visibility without row-object churn.
    ///
    /// # Errors
    ///
    /// Rejects a stale or unsupported row domain.
    #[wasm_bindgen(js_name = setDomainVisible)]
    pub fn set_domain_visible(
        &mut self,
        domain: &WebRowDomain,
        visible: bool,
    ) -> Result<bool, JsValue> {
        self.inner
            .set_domain_visible(domain.inner, visible)
            .map_err(core_error)
    }

    /// Returns the exact current row count for declarative introspection.
    #[must_use]
    #[wasm_bindgen(js_name = domainRowCount)]
    pub fn domain_row_count(&self, domain: &WebRowDomain) -> Option<u32> {
        self.inner.row_count(domain.inner)
    }
}

fn add_attribute(
    scene: &mut WebScene,
    domain: &WebRowDomain,
    name: String,
    values: molgfx::core::AttributeValues,
) -> Result<WebAttributeHandle, JsValue> {
    let column =
        molgfx::core::AttributeColumn::new(domain.inner, name, values).map_err(core_error)?;
    Ok(WebAttributeHandle {
        inner: scene.inner.add_attribute(column).map_err(core_error)?,
    })
}
