use super::{
    frames::Frames,
    types::{Bonds, SceneArchive, Surface},
};
use crate::{Error, RuntimeBindings, ScalarPropertyBinding, SceneSpec};
use std::collections::BTreeMap;

/// Shared immutable input captured for off-thread serialization.
/// Capture clones owners and authored metadata; it never scans or copies molecular columns.
#[derive(Clone, Debug)]
pub struct RuntimeCapture {
    pub(crate) scene: SceneSpec,
    pub(crate) properties: BTreeMap<Box<str>, ScalarPropertyBinding>,
    pub(crate) bindings: RuntimeBindings,
    pub(crate) retained: Vec<Self>,
}

impl RuntimeCapture {
    /// Materializes transported scientific assets in O(descriptors × log(descriptors) + total asset size), outside the render loop.
    /// Molecular source coordinates are excluded; resident frames are encoded once per owner.
    /// # Errors
    /// Refuses scientific assets whose authored identity cannot be found.
    pub fn archive(&self) -> Result<SceneArchive, Error> {
        self.archive_with_cancellation(&molframe::CancellationToken::default())
    }

    /// Materializes immutable assets with cooperative cancellation.
    /// # Errors
    /// Refuses inconsistent inputs or cancelled work; no partial archive is returned.
    pub fn archive_with_cancellation(
        &self,
        cancellation: &molframe::CancellationToken,
    ) -> Result<SceneArchive, Error> {
        super::control::check(cancellation)?;
        let mut frames = Frames::new(cancellation.clone());
        let mut archive = self.capture_assets(&mut frames, cancellation)?;
        for retained in &self.retained {
            archive
                .retained
                .push(retained.capture_assets(&mut frames, cancellation)?);
        }
        archive.frames = frames.data;
        Ok(archive)
    }

    fn capture_assets(
        &self,
        frames: &mut Frames,
        cancellation: &molframe::CancellationToken,
    ) -> Result<SceneArchive, Error> {
        let mut archive = SceneArchive {
            scene: self.scene.clone(),
            retained: Vec::new(),
            frames: Vec::new(),
            properties: self
                .properties
                .iter()
                .filter_map(|(name, binding)| {
                    if !frames.property(
                        binding.spec().structure.get(),
                        &binding.spec().source.content_hash,
                        name,
                    ) {
                        return None;
                    }
                    Some(
                        super::control::collect(
                            binding.values().iter().map(|value| value.to_bits()),
                            cancellation,
                        )
                        .map(|values| (name.clone(), values)),
                    )
                })
                .collect::<Result<_, Error>>()?,
            volumes: BTreeMap::new(),
            segmentations: BTreeMap::new(),
            hydrogen_bonds: BTreeMap::new(),
            surfaces: BTreeMap::new(),
            trajectories: BTreeMap::new(),
            playback: BTreeMap::new(),
        };
        self.capture_grids(&mut archive, frames, cancellation)?;
        self.capture_bonds(&mut archive, frames, cancellation)?;
        self.capture_surfaces(&mut archive, frames, cancellation)?;
        let trajectories: BTreeMap<_, _> = self
            .scene
            .trajectories
            .iter()
            .map(|(&id, spec)| (&*spec.source.content_hash, id))
            .collect();
        for binding in &self.bindings.trajectories {
            if let Some(&id) = trajectories.get(&*binding.source().content_hash)
                && let std::collections::btree_map::Entry::Vacant(entry) =
                    archive.trajectories.entry(id)
                && frames.asset(4, 0, &binding.source().content_hash)
            {
                let (start, end) = binding.frames();
                entry.insert(frames.pair(start, end, binding.sample_time())?);
            }
        }
        for binding in &self.bindings.playback {
            archive.playback.insert(
                binding.structure,
                frames.pair(&binding.start, &binding.end, binding.sample_seconds)?,
            );
        }
        Ok(archive)
    }

    fn capture_grids(
        &self,
        archive: &mut SceneArchive,
        frames: &mut Frames,
        cancellation: &molframe::CancellationToken,
    ) -> Result<(), Error> {
        let volumes: BTreeMap<_, _> = self
            .scene
            .volumes
            .iter()
            .map(|(&id, spec)| (&*spec.source.content_hash, id))
            .collect();
        for binding in &self.bindings.volumes {
            if let Some(&id) = volumes.get(&*binding.source().content_hash)
                && let std::collections::btree_map::Entry::Vacant(entry) = archive.volumes.entry(id)
                && frames.asset(0, 0, &binding.source().content_hash)
            {
                entry.insert(super::control::collect(
                    binding.values().iter().copied(),
                    cancellation,
                )?);
            }
        }
        let segmentations: BTreeMap<_, _> = self
            .scene
            .segmentations
            .iter()
            .map(|(&id, spec)| (&*spec.source.content_hash, id))
            .collect();
        for binding in &self.bindings.segmentations {
            if let Some(&id) = segmentations.get(binding.content_hash())
                && let std::collections::btree_map::Entry::Vacant(entry) =
                    archive.segmentations.entry(id)
                && frames.asset(1, 0, binding.content_hash())
            {
                entry.insert(super::control::collect(
                    binding.labels().iter().copied(),
                    cancellation,
                )?);
            }
        }
        Ok(())
    }

    fn capture_bonds(
        &self,
        archive: &mut SceneArchive,
        frames: &mut Frames,
        cancellation: &molframe::CancellationToken,
    ) -> Result<(), Error> {
        let bonds: BTreeMap<_, _> = self
            .scene
            .interactions
            .iter()
            .filter_map(|(&id, spec)| match spec {
                crate::InteractionSpec::HydrogenBonds(spec) => {
                    Some((&*spec.source.content_hash, (id, spec)))
                }
                crate::InteractionSpec::Explicit { .. } => None,
            })
            .collect();
        for binding in &self.bindings.hydrogen_bonds {
            if let Some(&(id, spec)) = bonds.get(&*binding.source().content_hash)
                && let std::collections::btree_map::Entry::Vacant(entry) =
                    archive.hydrogen_bonds.entry(id)
                && frames.asset(2, 0, &binding.source().content_hash)
            {
                let pose = binding.pose().ok_or_else(missing)?;
                entry.insert(Bonds {
                    source: spec.source.clone(),
                    structure: spec.structure,
                    rows: super::control::collect(binding.triples(), cancellation)?,
                    candidate_distance: binding.candidate_distance(),
                    interval: frames.interval(pose)?,
                });
            }
        }
        Ok(())
    }

    fn capture_surfaces(
        &self,
        archive: &mut SceneArchive,
        frames: &mut Frames,
        cancellation: &molframe::CancellationToken,
    ) -> Result<(), Error> {
        let surfaces: BTreeMap<_, _> = self
            .scene
            .representations
            .iter()
            .filter_map(|(&id, spec)| {
                let (source, _, _) = spec.molecular_surface_recipe()?;
                Some(((spec.structure_id()?, &*source.content_hash), id))
            })
            .collect();
        for binding in &self.bindings.molecular_surfaces {
            if let Some(&id) = surfaces.get(&(binding.structure, &*binding.source().content_hash))
                && let std::collections::btree_map::Entry::Vacant(entry) =
                    archive.surfaces.entry(id)
                && frames.asset(3, binding.structure.get(), &binding.source().content_hash)
            {
                entry.insert(Surface {
                    source: binding.source().clone(),
                    structure: binding.structure,
                    data: crate::MolecularSurfaceData {
                        method: binding.method(),
                        controls: binding.controls(),
                        mesh: super::control::surface(binding.geometry(), cancellation)?,
                    },
                    interval: binding
                        .geometry()
                        .pose()
                        .map(|pose| frames.interval(pose))
                        .transpose()?
                        .flatten(),
                });
            }
        }
        Ok(())
    }

    /// Borrowed authored state for checking a capture against a live host.
    #[must_use]
    pub const fn scene(&self) -> &SceneSpec {
        &self.scene
    }
}

fn missing() -> Error {
    Error::InvalidSpec("scientific capture has inconsistent authored input".to_owned())
}
