use super::{
    frames,
    types::{Interval, SceneArchive},
};
use crate::{
    Error, HydrogenBondBinding, MolecularSurfaceBinding, RuntimeBindings, ScalarPropertyBinding,
    Scene, StructureId, TrajectoryBinding, TrajectoryFrame,
};
use std::{collections::BTreeMap, sync::Arc};

impl SceneArchive {
    /// Validates and restores all scientific input before exposing a scene.
    /// Molecular sources must match the document's original content hashes.
    /// Stale calculations remain retained and become drawable only at their own sample.
    /// Call on a worker: geometry validation is O(total asset size).
    /// # Errors
    /// Refuses malformed frames, assets, ownership, descriptors or missing molecular inputs.
    pub fn restore(
        self,
        sources: BTreeMap<StructureId, crate::source::MolecularSource>,
    ) -> Result<Scene, Error> {
        self.restore_with_cancellation(sources, &molframe::CancellationToken::default())
    }

    /// Restores a detached scene with cancellable analytical geometry validation.
    /// # Errors
    /// Refuses contradictory scientific input, missing sources or cancelled work.
    pub fn restore_with_cancellation(
        mut self,
        sources: BTreeMap<StructureId, crate::source::MolecularSource>,
        cancellation: &molframe::CancellationToken,
    ) -> Result<Scene, Error> {
        super::control::check(cancellation)?;
        let Self {
            frames, retained, ..
        } = &mut self;
        let frames = frames::restore_frames(std::mem::take(frames))?;
        let retained = std::mem::take(retained);
        let mut scene = self.restore_assets(sources, &frames, cancellation, None)?;
        for archive in retained {
            if !archive.frames.is_empty() || !archive.retained.is_empty() {
                return Err(invalid());
            }
            let inherited = scene.capture_runtime_spec(&archive.scene);
            let retained = archive.restore_assets(
                scene.sources().clone(),
                &frames,
                cancellation,
                Some(inherited),
            )?;
            scene.absorb_archive_assets(retained)?;
        }
        Ok(scene)
    }

    fn restore_assets(
        self,
        sources: BTreeMap<StructureId, crate::source::MolecularSource>,
        frames: &[TrajectoryFrame],
        cancellation: &molframe::CancellationToken,
        inherited: Option<super::capture::RuntimeCapture>,
    ) -> Result<Scene, Error> {
        let (mut bindings, mut inherited_properties) = match inherited {
            Some(capture) => (capture.bindings, capture.properties),
            None => (RuntimeBindings::default(), BTreeMap::new()),
        };
        inherited_properties.extend(properties(&self.scene, self.properties)?);
        let properties = inherited_properties;
        let mut volume_sources = std::collections::BTreeSet::new();
        for (id, values) in self.volumes {
            let spec = self.scene.volumes.get(&id).ok_or_else(invalid)?;
            if !volume_sources.insert(&spec.source.content_hash) {
                return Err(invalid());
            }
            bindings.volumes.push(
                crate::VolumeBinding::new(spec.source.clone(), spec.dimensions, Arc::from(values))
                    .affine(spec.voxel_to_world),
            );
        }
        for (id, labels) in self.segmentations {
            let spec = self.scene.segmentations.get(&id).ok_or_else(invalid)?;
            bindings.segmentations.push(crate::SegmentationBinding::new(
                spec.source.clone(),
                spec.dimensions,
                spec.voxel_to_world,
                Arc::from(labels),
            )?);
        }
        for (id, data) in self.hydrogen_bonds {
            let Some(crate::InteractionSpec::HydrogenBonds(spec)) =
                self.scene.interactions.get(&id)
            else {
                return Err(invalid());
            };
            if spec.source != data.source || spec.structure != data.structure {
                return Err(invalid());
            }
            let source = sources.get(&data.structure).ok_or_else(invalid)?;
            let pose = pose(source, frames, data.interval)?;
            let binding = match data.candidate_distance {
                Some(distance) => {
                    HydrogenBondBinding::candidates(data.source, Arc::from(data.rows), distance)
                }
                None => HydrogenBondBinding::new(data.source, Arc::from(data.rows)),
            }
            .at_pose(pose);
            bindings.hydrogen_bonds.push(binding);
        }
        for (id, data) in self.surfaces {
            let spec = self.scene.representations.get(&id).ok_or_else(invalid)?;
            if spec.structure_id() != Some(data.structure)
                || spec
                    .molecular_surface_recipe()
                    .is_none_or(|(source, _, _)| *source != data.source)
            {
                return Err(invalid());
            }
            let source = sources.get(&data.structure).ok_or_else(invalid)?;
            let pose = pose(source, frames, data.interval)?;
            bindings.molecular_surfaces.push(
                MolecularSurfaceBinding::new(
                    data.source,
                    data.structure,
                    source,
                    data.data.method,
                    data.data.controls,
                    data.data
                        .mesh
                        .into_geometry_with_cancellation(cancellation)?,
                )?
                .at_pose(&pose)?,
            );
        }
        for (id, interval) in self.trajectories {
            let spec = self.scene.trajectories.get(&id).ok_or_else(invalid)?;
            let (start, end) = frames::pair(frames, interval)?;
            bindings.trajectories.push(
                TrajectoryBinding::new(spec.source.clone(), start, end)
                    .sample_seconds(interval.sample),
            );
        }
        for (structure, interval) in self.playback {
            let (start, end) = frames::pair(frames, interval)?;
            bindings.playback.push(crate::PlaybackBinding {
                structure,
                start,
                end,
                sample_seconds: interval.sample,
            });
        }
        super::control::check(cancellation)?;
        let scene = Scene::from_spec_with_bindings(self.scene, sources, properties, bindings)?;
        super::control::check(cancellation)?;
        Ok(scene)
    }
}

fn properties(
    scene: &crate::SceneSpec,
    values: BTreeMap<Box<str>, Vec<u32>>,
) -> Result<BTreeMap<Box<str>, ScalarPropertyBinding>, Error> {
    let mut properties = BTreeMap::new();
    for (name, words) in values {
        let spec = scene.properties.get(&name).ok_or_else(invalid)?;
        let mut binding = ScalarPropertyBinding::new(
            spec.structure,
            name.clone(),
            spec.source.clone(),
            Arc::from(words.into_iter().map(f32::from_bits).collect::<Vec<_>>()),
        )
        .domain(spec.domain);
        if let Some(units) = &spec.units {
            binding = binding.units(units.clone());
        }
        properties.insert(name, binding);
    }
    Ok(properties)
}

fn pose(
    source: &crate::source::MolecularSource,
    frames: &[TrajectoryFrame],
    interval: Option<Interval>,
) -> Result<crate::source::MolecularPose, Error> {
    match interval {
        Some(interval) => {
            let (start, end) = frames::pair(frames, interval)?;
            TrajectoryBinding::new(crate::DataSource::new("archive-frame-pair"), start, end)
                .sample_seconds(interval.sample)
                .capture_pose(source)
        }
        None => Ok(crate::source::MolecularPose::new(source.clone(), None)?),
    }
}

fn invalid() -> Error {
    Error::InvalidSpec(
        "scientific archive has contradictory authored identities or missing data".to_owned(),
    )
}
