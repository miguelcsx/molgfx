//! Declarative visual programs attached to exact generic row domains.

#[cfg(test)]
#[path = "domain_visuals_tests.rs"]
mod tests;

use crate::{
    CoreError, RowDomain, Scene, VisualAttributeRef, VisualCompatibility, VisualDescriptor,
};

impl Scene {
    /// Atomically validates and attaches one generic visual descriptor.
    ///
    /// # Errors
    ///
    /// The domain must be live, every attribute must target that exact domain,
    /// and all outputs must be supported by the drawable family.
    pub fn set_domain_visual(
        &mut self,
        domain: RowDomain,
        descriptor: VisualDescriptor,
    ) -> Result<Option<VisualDescriptor>, CoreError> {
        self.row_count(domain).ok_or(CoreError::StaleHandle)?;
        for reference in descriptor.style().program().attributes() {
            let VisualAttributeRef::Attribute { handle, kind } = *reference else {
                return Err(invalid(
                    "generic domain visuals cannot reference legacy atom properties",
                ));
            };
            let attribute = self.attribute(handle).ok_or(CoreError::StaleHandle)?;
            if attribute.domain() != domain || attribute.kind() != kind {
                return Err(invalid(
                    "visual attributes must target the exact descriptor row domain",
                ));
            }
        }
        descriptor
            .style()
            .program()
            .validate_compatibility(compatibility(domain))
            .map_err(|error| CoreError::InvalidVisual {
                summary: error.to_string(),
            })?;
        let previous = self.domain_visuals.insert(domain, descriptor);
        self.domain_visual_revision = self.domain_visual_revision.wrapping_add(1);
        Ok(previous)
    }

    /// Exact visual descriptor for one generic row domain.
    #[must_use]
    pub fn domain_visual(&self, domain: RowDomain) -> Option<&VisualDescriptor> {
        self.domain_visuals.get(&domain)
    }

    /// Mutable descriptor access with eager revision invalidation.
    pub fn domain_visual_mut(&mut self, domain: RowDomain) -> Option<&mut VisualDescriptor> {
        if !self.domain_visuals.contains_key(&domain) {
            return None;
        }
        self.domain_visual_revision = self.domain_visual_revision.wrapping_add(1);
        self.domain_visuals.get_mut(&domain)
    }

    /// Removes one visual attachment without touching its immutable inputs.
    pub fn remove_domain_visual(&mut self, domain: RowDomain) -> Option<VisualDescriptor> {
        let removed = self.domain_visuals.remove(&domain);
        if removed.is_some() {
            self.domain_visual_revision = self.domain_visual_revision.wrapping_add(1);
        }
        removed
    }

    /// Stable domain-ordered introspection for a future Studio.
    pub fn domain_visuals(&self) -> impl Iterator<Item = (RowDomain, &VisualDescriptor)> + '_ {
        self.domain_visuals
            .iter()
            .map(|(domain, descriptor)| (*domain, descriptor))
    }

    /// Membership, program, parameter or order revision.
    #[must_use]
    pub const fn domain_visual_revision(&self) -> u64 {
        self.domain_visual_revision
    }
}

const fn compatibility(domain: RowDomain) -> VisualCompatibility {
    match domain {
        RowDomain::Points(_) => VisualCompatibility::POINTS,
        RowDomain::Instances(_) | RowDomain::TemplateParts(_) => VisualCompatibility::INSTANCES,
        RowDomain::Relations(_) => VisualCompatibility::RELATIONS,
        RowDomain::Atoms(_) => VisualCompatibility::DEFORMABLE,
    }
}

fn invalid(reason: &'static str) -> CoreError {
    CoreError::InvalidVisual {
        summary: reason.into(),
    }
}
