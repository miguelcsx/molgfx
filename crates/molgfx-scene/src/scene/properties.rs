//! Validation and physical resolution for scalar property bindings.

use super::Scene;
use crate::color::DerivedColumn;
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
        let reference = self.attach_property(binding)?;
        self.spec.revision = self.spec.revision.wrapping_add(1);
        Ok(reference)
    }

    /// Validates and installs one property binding without touching the
    /// revision, so implicit bindings do not advance the patch stream.
    fn attach_property(&mut self, binding: ScalarPropertyBinding) -> Result<ScalarProperty, Error> {
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
        Ok(reference)
    }

    /// Binds every derived column the colours in `patch` read, once.
    ///
    /// A derived column is a pure function of its structure, so binding it is
    /// idempotent and adds no revision: a replica that applies the same patch
    /// derives the same columns and reaches the same state.
    pub(crate) fn ensure_derived(&mut self, patch: &crate::ScenePatch) -> Result<(), Error> {
        let mut wanted: Vec<(StructureId, DerivedColumn)> = Vec::new();
        // Structures of representations the patch itself adds, which a later
        // operation in the same patch may recolour before the scene knows them.
        let mut added: BTreeMap<crate::RepresentationId, Option<StructureId>> = BTreeMap::new();
        let only = self.only_structure();
        for operation in &patch.operations {
            use crate::PatchOperation::{
                AddAppearanceRule, AddRepresentation, ReplaceAppearanceRule, ReplaceRepresentation,
                SetColor,
            };
            let (structure, color) = match operation {
                AddRepresentation { id, representation }
                | ReplaceRepresentation { id, representation } => {
                    let structure = representation.structure_id().or(only);
                    let _ = added.insert(*id, structure);
                    (structure, representation.color())
                }
                SetColor { id, color } => {
                    let structure = match added.get(id) {
                        Some(structure) => *structure,
                        None => self
                            .spec
                            .representations
                            .get(id)
                            .and_then(crate::RepresentationSpec::structure_id),
                    };
                    (structure, color)
                }
                AddAppearanceRule { rule, .. } | ReplaceAppearanceRule { rule, .. } => {
                    (Some(rule.structure), &rule.color)
                }
                _ => continue,
            };
            if let (Some(structure), Some(column)) = (structure, color.derived()) {
                wanted.push((structure, column));
            }
        }
        for (structure, column) in wanted {
            self.bind_derived(structure, column)?;
        }
        Ok(())
    }

    /// The scene's only structure, when it has exactly one.
    fn only_structure(&self) -> Option<StructureId> {
        let mut ids = self.structures.keys();
        match (ids.next(), ids.next()) {
            (Some(id), None) => Some(*id),
            _ => None,
        }
    }

    /// Derives `column` for `structure` and binds it, unless it already is.
    fn bind_derived(&mut self, structure: StructureId, column: DerivedColumn) -> Result<(), Error> {
        let name = column.property_name(structure).into_boxed_str();
        if self.property_bindings.contains_key(&name) {
            return Ok(());
        }
        let source = self
            .structures
            .get(&structure)
            .ok_or_else(|| Error::InvalidSpec("derived column owner is not bound".to_owned()))?;
        let binding = column.binding(structure, source)?;
        self.attach_property(binding)?;
        Ok(())
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
