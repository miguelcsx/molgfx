use super::types::{Frame, Interval};
use crate::TrajectoryFrame;
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::Arc,
};

pub(super) struct Frames {
    pub(super) data: Vec<Frame>,
    seen: BTreeMap<(u64, u32, usize), usize>,
    cancellation: molframe::CancellationToken,
    assets: BTreeSet<(u8, u64, Box<str>, Box<str>)>,
}

impl Frames {
    pub(super) fn new(cancellation: molframe::CancellationToken) -> Self {
        Self {
            data: Vec::new(),
            seen: BTreeMap::new(),
            cancellation,
            assets: BTreeSet::new(),
        }
    }

    pub(super) fn asset(&mut self, kind: u8, structure: u64, hash: &str) -> bool {
        self.assets
            .insert((kind, structure, hash.into(), "".into()))
    }

    pub(super) fn property(&mut self, structure: u64, hash: &str, name: &str) -> bool {
        self.assets.insert((5, structure, hash.into(), name.into()))
    }

    fn insert(
        &mut self,
        index: u64,
        time: f32,
        positions: &[[f32; 3]],
    ) -> Result<usize, crate::Error> {
        let key = (index, time.to_bits(), positions.as_ptr() as usize);
        if let Some(&existing) = self.seen.get(&key) {
            return Ok(existing);
        }
        let id = self.data.len();
        self.data.push(Frame {
            index,
            time,
            positions: super::control::collect(positions.iter().copied(), &self.cancellation)?,
        });
        self.seen.insert(key, id);
        Ok(id)
    }

    pub(super) fn interval(
        &mut self,
        pose: &crate::source::MolecularPose,
    ) -> Result<Option<Interval>, crate::Error> {
        let Some(segment) = pose.trajectory() else {
            return Ok(None);
        };
        Ok(Some(Interval {
            start: self.insert(
                segment.start().index(),
                segment.start().time_seconds(),
                segment.start().positions(),
            )?,
            end: self.insert(
                segment.end().index(),
                segment.end().time_seconds(),
                segment.end().positions(),
            )?,
            sample: segment.sample_seconds(),
        }))
    }

    pub(super) fn pair(
        &mut self,
        start: &TrajectoryFrame,
        end: &TrajectoryFrame,
        sample: f32,
    ) -> Result<Interval, crate::Error> {
        Ok(Interval {
            start: self.insert(start.index(), start.time_seconds(), start.positions())?,
            end: self.insert(end.index(), end.time_seconds(), end.positions())?,
            sample,
        })
    }
}

pub(super) fn restore_frames(frames: Vec<Frame>) -> Result<Vec<TrajectoryFrame>, crate::Error> {
    frames
        .into_iter()
        .map(|frame| {
            let frame = TrajectoryFrame::new(frame.index, frame.time, Arc::from(frame.positions));
            frame.native()?;
            Ok(frame)
        })
        .collect()
}

pub(super) fn pair(
    frames: &[TrajectoryFrame],
    interval: Interval,
) -> Result<(TrajectoryFrame, TrajectoryFrame), crate::Error> {
    let start = frames.get(interval.start).ok_or_else(invalid)?;
    let end = frames.get(interval.end).ok_or_else(invalid)?;
    Ok((start.clone(), end.clone()))
}

fn invalid() -> crate::Error {
    crate::Error::InvalidSpec("scientific archive references a missing coordinate frame".to_owned())
}
