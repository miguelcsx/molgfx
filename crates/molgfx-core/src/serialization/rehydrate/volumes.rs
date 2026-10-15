use super::{insert, invalid, raw, resolve_structure, same_array_bits, value_hash};
use crate::scene::{BoundOccupancy, Scene, StoredVolume};
use crate::serialization::types::{OccupancyDescription, VolumeDescription};
use crate::{CoreError, StructureHandle};

pub(super) fn rehydrate(
    scene: &mut Scene,
    descriptions: &[VolumeDescription],
    sources: &[crate::ScalarVolume],
    structures: &[StructureHandle],
) -> Result<(), CoreError> {
    let mut sources = sources.iter();
    for description in descriptions {
        if let Some(occupancy) = &description.occupancy {
            rehydrate_occupancy(scene, description, occupancy, structures)?;
            continue;
        }
        let Some(value) = sources.next() else {
            return invalid("scalar volume source is absent");
        };
        if value.dimensions() != description.dimensions
            || !same_array_bits(value.range(), description.range)
            || !same_array_bits(
                value.voxel_to_world().to_cols_array(),
                description.voxel_to_world,
            )
            || value_hash(value.values()) != description.content_hash
        {
            return invalid("supplied scalar volume does not match its manifest");
        }
        let raw = raw(description.row, description.generation);
        insert(
            scene.volumes.insert_at(
                raw,
                StoredVolume {
                    value: Some(value.clone()),
                    occupancy: None,
                    revision: 0,
                },
            ),
            "volume identity collision",
        )?;
    }
    Ok(())
}

fn rehydrate_occupancy(
    scene: &mut Scene,
    description: &VolumeDescription,
    occupancy: &OccupancyDescription,
    structures: &[StructureHandle],
) -> Result<(), CoreError> {
    let structure = resolve_structure(structures, occupancy.structure)?;
    let transform = molgfx_math::Mat4::from_cols_array(&occupancy.voxel_to_model);
    let spacing = molgfx_math::Vec3::new(
        transform.x_axis.truncate().length(),
        transform.y_axis.truncate().length(),
        transform.z_axis.truncate().length(),
    );
    let stream = crate::OccupancyStream::new(
        description.dimensions,
        transform.w_axis.truncate(),
        spacing,
        occupancy.decay,
        occupancy.deposit,
        occupancy.maximum,
    )?;
    let atom_count = scene
        .structure(structure)
        .map_or(0, |placed| placed.atoms.len());
    if occupancy
        .atom_rows
        .windows(2)
        .any(|rows| rows[0] >= rows[1])
        || occupancy.atom_rows.iter().any(|&row| row >= atom_count)
    {
        return invalid("occupancy atom rows are invalid");
    }
    let raw = raw(description.row, description.generation);
    insert(
        scene.volumes.insert_at(
            raw,
            StoredVolume {
                value: None,
                occupancy: Some(BoundOccupancy {
                    stream,
                    structure,
                    atom_rows: std::sync::Arc::from(occupancy.atom_rows.clone()),
                }),
                revision: 0,
            },
        ),
        "volume identity collision",
    )
}
