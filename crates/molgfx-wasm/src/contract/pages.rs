//! Paged entry points for large batch inputs.
//!
//! JavaScript fills a typed page accumulator in bounded slices, then hands the
//! finished column to the scene by reference. Nothing here holds a second
//! whole-column copy: the accumulator's allocation is the column.

use super::{WebScene, javascript_error};
use crate::paging::Paged;
use wasm_bindgen::prelude::*;

macro_rules! page_class {
    ($class:ident, $js:literal, $element:ty) => {
        #[wasm_bindgen(js_name = $js)]
        #[derive(Debug)]
        /// A typed column filled in bounded pages against a declared length.
        pub struct $class(Paged<$element>);

        #[wasm_bindgen(js_class = $js)]
        impl $class {
            /// Reserves the whole column once.
            ///
            /// # Errors
            /// Returns an error when the declared length cannot be reserved.
            #[wasm_bindgen(constructor)]
            pub fn new(total: usize) -> Result<$class, JsError> {
                Paged::new(total).map(Self).map_err(javascript_error)
            }

            /// Appends one page in order.
            ///
            /// # Errors
            /// Returns an error when the page passes the declared length.
            pub fn push(&mut self, page: &[$element]) -> Result<(), JsError> {
                self.0.push(page).map_err(javascript_error)
            }
        }
    };
}

page_class!(FloatPages, "FloatPages", f32);
page_class!(WordPages, "WordPages", u32);
page_class!(BytePages, "BytePages", u8);

#[wasm_bindgen(js_class = Scene)]
impl WebScene {
    /// Parses paged local molecular bytes into a resolved one-structure scene.
    ///
    /// # Errors
    /// Returns an error for incomplete pages or unparsable data.
    #[wasm_bindgen(js_name = fromStructurePages)]
    pub fn from_structure_pages(
        pages: &mut BytePages,
        name: Option<String>,
    ) -> Result<WebScene, JsError> {
        Self::from_structure_bytes(pages.0.finish().map_err(javascript_error)?, name)
    }

    /// Binds one paged molecular source to its semantic identity.
    ///
    /// # Errors
    /// Returns an error for incomplete pages, an undeclared identity or bad data.
    #[wasm_bindgen(js_name = bindStructurePaged)]
    pub fn bind_structure_paged(
        &mut self,
        identity: u64,
        pages: &mut BytePages,
        name: Option<String>,
    ) -> Result<(), JsError> {
        self.bind_structure(identity, pages.0.finish().map_err(javascript_error)?, name)
    }

    /// Restores one paged volume grid.
    ///
    /// # Errors
    /// Returns an error for incomplete pages or an invalid grid.
    #[wasm_bindgen(js_name = restoreVolumePaged)]
    pub fn restore_volume_paged(
        &mut self,
        identity: u64,
        pages: &mut FloatPages,
    ) -> Result<(), JsError> {
        self.restore_volume(identity, pages.0.finish().map_err(javascript_error)?)
    }

    /// Stages one paged coordinate column.
    ///
    /// # Errors
    /// Returns an error for incomplete pages or invalid coordinates.
    #[wasm_bindgen(js_name = restoreCoordinateFramePaged)]
    pub fn restore_coordinate_frame_paged(
        &mut self,
        identity: u64,
        index: u64,
        seconds: f32,
        pages: &FloatPages,
    ) -> Result<(), JsError> {
        if !pages.0.is_complete() {
            return Err(JsError::new("coordinate pages are incomplete"));
        }
        self.restore_coordinate_frame(identity, index, seconds, pages.0.as_slice())
    }

    /// Stages paged hydrogen-bond address triples.
    ///
    /// # Errors
    /// Returns an error for incomplete pages or invalid presentation data.
    #[wasm_bindgen(js_name = restoreHydrogenBondsPaged)]
    pub fn restore_hydrogen_bonds_paged(
        &mut self,
        identity: u64,
        pages: &WordPages,
        candidate_distance: Option<f32>,
        pose: Option<String>,
    ) -> Result<(), JsError> {
        if !pages.0.is_complete() {
            return Err(JsError::new("hydrogen-bond pages are incomplete"));
        }
        self.restore_hydrogen_bonds(identity, pages.0.as_slice(), candidate_distance, pose)
    }
}
