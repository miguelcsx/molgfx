//! The sealed set of values a scene accepts.

use crate::representation::RepresentationSpec;

pub(crate) mod private {
    pub trait Sealed {}
}

/// A value accepted by [`crate::Scene::add`].
pub trait SceneItem: private::Sealed {
    /// Stable semantic identity returned by [`crate::Scene::add`].
    type Id;

    #[doc(hidden)]
    fn add_to(self, scene: &mut crate::Scene) -> Result<Self::Id, crate::Error>;
}

impl private::Sealed for RepresentationSpec {}

impl SceneItem for RepresentationSpec {
    type Id = crate::RepresentationId;

    fn add_to(self, scene: &mut crate::Scene) -> Result<Self::Id, crate::Error> {
        scene.insert_representation(self)
    }
}
