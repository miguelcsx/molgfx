//! Explicit and bulk chemically classified interactions share one lowering seam.

use super::InteractionSpec;
use super::lower::{LoweredOverlay, OverlayLowering, owner_of, positions};
use crate::Error;
use molgfx_core::{InteractionAnchor, InteractionEdge, InteractionGeometry};

pub(super) fn lower_interactions(
    spec: &crate::SceneSpec,
    lowering: &mut OverlayLowering<'_>,
    lowered: &mut LoweredOverlay,
) -> Result<(), Error> {
    for (id, interaction) in &spec.interactions {
        let (kind, endpoints) = match interaction {
            InteractionSpec::Explicit { kind, endpoints } => (kind, endpoints),
            InteractionSpec::HydrogenBonds(spec) => {
                if let Some(handle) = super::hydrogen_bonds::lower(spec, lowering)? {
                    lowered.interaction_batches.push((*id, handle));
                }
                continue;
            }
        };
        let points = positions(
            lowering.scene,
            lowering.structures,
            lowering.handles,
            lowering.selections,
            endpoints,
        )?;
        let owner = owner_of(lowering.scene, lowering.handles, endpoints)?;
        let distance = (points[1] - points[0]).length();
        if !distance.is_finite() || distance <= 0.0 {
            return Err(Error::InvalidSpec(
                "interaction endpoints must resolve to distinct finite positions".to_owned(),
            ));
        }
        let mut dependencies = [None, None];
        for (dependency, anchor) in dependencies.iter_mut().zip(endpoints) {
            if let super::Anchor::Selection { structure, .. } = anchor {
                *dependency = lowering.handles.get(structure).copied();
            }
        }
        let edge = InteractionEdge::new(
            owner,
            InteractionAnchor::world(points[0])?,
            InteractionAnchor::world(points[1])?,
            kind.core_kind(),
            InteractionGeometry::new(distance, None)?,
            "molgfx:explicit",
        )?
        .with_static_coordinates(dependencies, lowering.scene)?;
        lowered
            .interactions
            .push((*id, lowering.scene.add_interaction(edge)?));
    }

    Ok(())
}
