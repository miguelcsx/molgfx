//! Property colours against the scene's bound properties.

use crate::error::{CommandError, ErrorKind};
use crate::ir::ColorValue;
use crate::registry;
use molgfx_api::{Color, ColorSpec, SceneSpec, StructureId};

/// Colour for values a property does not have.
const MISSING: Color = Color::rgb(160, 160, 160);

/// The declarative colour of a property colouring over `structure`.
pub(crate) fn color(
    scene: &SceneSpec,
    color: &ColorValue,
    structure: StructureId,
) -> Result<ColorSpec, CommandError> {
    let ColorValue::Property {
        property,
        ramp,
        domain,
    } = color
    else {
        return Err(CommandError::new(
            ErrorKind::InvalidColor,
            format!("'{color}' is not a property colour"),
        ));
    };
    let Some(spec) = scene.properties.get(property) else {
        return Err(CommandError::new(
            ErrorKind::UnknownSymbol,
            format!("there is no bound property named '{property}'"),
        )
        .suggest(registry::suggest(
            property,
            scene.properties.keys().map(AsRef::as_ref),
        )));
    };
    if spec.structure != structure {
        return Err(CommandError::new(
            ErrorKind::InvalidColor,
            format!("property '{property}' belongs to another structure"),
        ));
    }
    let reference = serde_json::json!({ "structure": structure, "name": property });
    let property = serde_json::from_value(reference)
        .map_err(|error| CommandError::new(ErrorKind::InvalidColor, error.to_string()))?;
    Ok(molgfx_api::color::property(
        property,
        ramp.clone(),
        match domain {
            Some(domain) => *domain,
            None => spec.domain,
        },
        spec.units.clone(),
        MISSING,
    ))
}
