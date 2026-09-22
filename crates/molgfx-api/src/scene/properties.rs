//! Validation and physical resolution for scalar property bindings.

use super::Scene;
use crate::{Error, ScalarProperty, ScalarPropertyBinding, SceneSpec, StructureId};
use molgfx_core::{AtomPropertyHandle, StructureHandle};
use std::collections::BTreeMap;
use std::sync::Arc;

impl Scene {
    /// Binds one shared atom-row scalar column without copying its values.
    ///
    /// # Errors
    ///
    /// Returns an invalid-specification error for unknown ownership, duplicate
    /// names, mismatched rows, invalid domains or units, infinities, or a
    /// column containing only missing values.
    pub fn bind_property(
        &mut self,
        binding: ScalarPropertyBinding,
    ) -> Result<ScalarProperty, Error> {
        let name = binding.name().to_owned().into_boxed_str();
        if self.property_bindings.contains_key(&name) {
            return Err(Error::InvalidSpec(format!(
                "property '{name}' is already bound"
            )));
        }
        let structure_id = binding.spec().structure;
        let structure = self
            .structures
            .get(&structure_id)
            .ok_or_else(|| Error::InvalidSpec("property owner is not bound".to_owned()))?;
        binding.validate(atom_rows(structure))?;
        let owner = structure_handle(&self.structures, &self.resolved, structure_id)?;
        let property = native_property(owner, &binding)?;
        let handle = self.resolved.add_atom_property(property)?;
        let (reference, spec, _) = binding.clone().into_parts();
        let _ = self.spec.properties.insert(name.clone(), spec);
        let _ = self.properties.insert(name.clone(), handle);
        let _ = self.property_bindings.insert(name, binding);
        self.spec.revision = self.spec.revision.wrapping_add(1);
        Ok(reference)
    }
}

pub(crate) fn resolve_bindings(
    spec: &SceneSpec,
    structures: &BTreeMap<StructureId, molgfx_core::MolecularSource>,
    bindings: &BTreeMap<Box<str>, ScalarPropertyBinding>,
    structure_handles: &BTreeMap<StructureId, StructureHandle>,
    scene: &mut molgfx_core::Scene,
) -> Result<BTreeMap<Box<str>, AtomPropertyHandle>, Error> {
    if spec.properties.len() != bindings.len() {
        return Err(Error::InvalidSpec(
            "property bindings must exactly match SceneSpec descriptors".to_owned(),
        ));
    }
    let mut handles = BTreeMap::new();
    for (name, binding) in bindings {
        let descriptor = spec.properties.get(name).ok_or_else(|| {
            Error::InvalidSpec(format!("property binding '{name}' has no descriptor"))
        })?;
        if descriptor != binding.spec() || name.as_ref() != binding.name() {
            return Err(Error::InvalidSpec(format!(
                "property binding '{name}' does not match its descriptor"
            )));
        }
        let structure = structures.get(&descriptor.structure).ok_or_else(|| {
            Error::InvalidSpec(format!("property '{name}' owns an unknown structure"))
        })?;
        binding.validate(atom_rows(structure))?;
        let owner = structure_handles
            .get(&descriptor.structure)
            .copied()
            .ok_or_else(|| Error::InvalidSpec("property owner is unresolved".to_owned()))?;
        let handle = scene.add_atom_property(native_property(owner, binding)?)?;
        let _ = handles.insert(name.clone(), handle);
    }
    Ok(handles)
}

fn native_property(
    owner: StructureHandle,
    binding: &ScalarPropertyBinding,
) -> Result<molgfx_core::AtomProperty, Error> {
    let semantics = binding.spec().units.as_ref().map_or_else(
        || Ok(molgfx_core::ScalarFieldSemantics::UncalibratedRank),
        |units| {
            molgfx_core::ScalarFieldSemantics::quantity(
                Arc::from(binding.name()),
                Arc::from(units.as_ref()),
                Arc::from(binding.spec().source.content_hash.as_ref()),
            )
            .map_err(Error::from)
        },
    )?;
    molgfx_core::AtomProperty::new(
        owner,
        Arc::from(binding.name()),
        Arc::clone(binding.values()),
        molgfx_core::AtomPropertyMeaning::Generic,
        semantics,
    )
    .map_err(Error::from)
}

fn structure_handle(
    structures: &BTreeMap<StructureId, molgfx_core::MolecularSource>,
    scene: &molgfx_core::Scene,
    target: StructureId,
) -> Result<StructureHandle, Error> {
    structures
        .keys()
        .copied()
        .zip(scene.structures().map(|(handle, _)| handle))
        .find_map(|(id, handle)| (id == target).then_some(handle))
        .ok_or_else(|| Error::InvalidSpec("property owner is unresolved".to_owned()))
}

fn atom_rows(structure: &molgfx_core::MolecularSource) -> usize {
    structure.coordinates().len()
}
