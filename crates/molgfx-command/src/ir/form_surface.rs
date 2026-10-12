//! The form that describes an analytical surface a provider has computed.

use super::{Form, FormKind, OptionError};
use molframe::surface::native_solver::{NativeSurfaceMesh, NativeSurfaceProvider};

impl Form {
    /// The `surf` or `msms` form whose controls record how `mesh` was made,
    /// citing `source` as the content hash of the geometry it draws.
    ///
    /// # Errors
    ///
    /// Returns an [`OptionError`] when a recorded control is not valid for the
    /// form, which a provider's own options never are.
    pub fn from_native_surface(
        source: &str,
        mesh: &NativeSurfaceMesh,
    ) -> Result<Self, OptionError> {
        let kind = match mesh.provider() {
            NativeSurfaceProvider::Surf => FormKind::Surf,
            NativeSurfaceProvider::Msms => FormKind::Msms,
        };
        let options = mesh.options();
        let mut form = Self::new(kind);
        form.set_option("source", source)?;
        form.set_option("probe_radius", &options.probe_radius.to_string())?;
        if kind == FormKind::Msms {
            form.set_option("density", &options.density.to_string())?;
            form.set_option("all_components", &options.all_components.to_string())?;
        }
        Ok(form)
    }
}
