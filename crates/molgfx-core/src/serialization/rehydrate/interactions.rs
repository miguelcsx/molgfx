//! Restores scientific edges and their exact static coordinate dependencies.
use super::payload::{
    owner, parse_interaction_anchor, parse_interaction_direction, parse_interaction_kind,
};
use super::{Scene, insert};
use crate::{InteractionEdge, InteractionGeometry, serialization::InteractionDescription};

pub(crate) fn rehydrate_interactions(
    scene: &mut Scene,
    descriptions: &[InteractionDescription],
) -> Result<(), crate::CoreError> {
    for description in descriptions {
        let owning_structure = owner(scene, description.owner)?;
        let mut value = InteractionEdge::new(
            owning_structure,
            parse_interaction_anchor(scene, &description.start)?,
            parse_interaction_anchor(scene, &description.end)?,
            parse_interaction_kind(&description.kind)?,
            InteractionGeometry::new(description.distance_angstrom, description.angle_degrees)?,
            description.provenance.clone(),
        )?
        .with_direction(parse_interaction_direction(&description.direction)?);
        if let Some(occupancy) = description.occupancy {
            value = value.with_occupancy(occupancy)?;
        }
        if let Some(strength) = description.normalized_strength {
            value = value.with_normalized_strength(strength)?;
        }
        value = value.with_phase_speed(description.phase_speed_pixels_per_frame)?;
        value = value.with_persistence(
            description.persistence_age_frames,
            description.persistence_half_life_frames,
        )?;
        value.set_visible(description.visible);
        let mut coordinates = [None, None];
        for (target, input) in coordinates.iter_mut().zip(&description.static_coordinates) {
            if let Some((structure, transform)) = input {
                *target = Some((
                    owner(scene, *structure)?,
                    molgfx_math::Mat4::from_cols_array(transform),
                ));
            }
        }
        value = value.with_static_coordinates_at(coordinates, scene)?;
        insert(
            scene.interactions.insert_at(
                crate::serialization::rehydrate::raw(description.row, description.generation),
                value,
            ),
            "interaction identity collision",
        )?;
    }
    Ok(())
}
