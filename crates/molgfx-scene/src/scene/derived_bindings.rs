//! Property bindings a derived color column implies.

use crate::error::Error;
use crate::id::StructureId;
use crate::spec::SceneSpec;
use std::collections::BTreeMap;

/// Adds a binding for every derived property the specification declares.
///
/// Callers supply only the columns they own; a derived column is rebuilt from
/// its structure, so a serialized scene carries a descriptor and no values.
pub(super) fn with_derived_bindings(
    spec: &SceneSpec,
    structures: &BTreeMap<StructureId, molgfx_core::MolecularSource>,
    mut bindings: BTreeMap<Box<str>, crate::ScalarPropertyBinding>,
) -> Result<BTreeMap<Box<str>, crate::ScalarPropertyBinding>, Error> {
    for (name, descriptor) in &spec.properties {
        let derived = descriptor.source.format.as_deref() == Some(crate::color::DERIVED_FORMAT);
        if !derived || bindings.contains_key(name) {
            continue;
        }
        let column = descriptor
            .source
            .uri
            .as_deref()
            .and_then(crate::color::DerivedColumn::from_name)
            .ok_or_else(|| {
                Error::InvalidSpec(format!("derived property '{name}' names an unknown column"))
            })?;
        let source = structures.get(&descriptor.structure).ok_or_else(|| {
            Error::InvalidSpec(format!(
                "derived property '{name}' owns an unknown structure"
            ))
        })?;
        let _ = bindings.insert(name.clone(), column.binding(descriptor.structure, source)?);
    }
    Ok(bindings)
}
