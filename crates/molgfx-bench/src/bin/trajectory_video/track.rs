//! Trajectory feeds: a first pass that reads every frame time and bound, and a
//! forward-only cursor that holds only the two frames playback is between.

use std::error::Error;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use molframe::trajectory::{Timestep, TrajectoryReader, TrajectoryReaderOptions, read_trajectory};
use molgfx::TrajectoryFrame;

/// Seconds per picosecond: molframe reports frame times in picoseconds.
const PICOSECOND: f64 = 1.0e-12;

/// One trajectory file and the structure it drives.
#[derive(Clone, Debug)]
pub(crate) struct Feed {
    pub(crate) path: PathBuf,
    /// The file atom behind each structure atom. `None` means the file lists the
    /// structure's atoms in the same order.
    pub(crate) map: Option<Box<[usize]>>,
    /// Atoms per frame in the file.
    pub(crate) file_atoms: usize,
    /// Atoms per frame in the structure, which is what the renderer receives.
    pub(crate) atoms: usize,
}

impl Feed {
    /// The file atom behind structure atom `atom`.
    fn file_index(&self, atom: usize) -> usize {
        self.map.as_ref().map_or(atom, |map| map[atom])
    }
}

/// Atoms per frame in the trajectory at `path`.
pub(crate) fn file_atoms(path: &Path) -> Result<usize, Box<dyn Error>> {
    Ok(read_trajectory(path, &TrajectoryReaderOptions::default())?.n_atoms())
}

/// What the first pass learns about a feed: every frame time, and the box that holds
/// the structure's atoms in every frame.
pub(crate) struct Scan {
    /// Frame times in seconds, strictly ascending.
    pub(crate) times: Vec<f64>,
    /// Lower corner of the box, in the file's coordinates.
    pub(crate) min: [f32; 3],
    /// Upper corner of the box, in the file's coordinates.
    pub(crate) max: [f32; 3],
}

/// Reads every frame of a feed once, keeping only the times and the bounds.
///
/// # Errors
/// Returns an error when the file cannot be read, has no frames, or has frame times
/// that do not increase.
pub(crate) fn scan(feed: &Feed) -> Result<Scan, Box<dyn Error>> {
    let mut reader = open(feed)?;
    let mut timestep = Timestep::default();
    let mut times: Vec<f64> = Vec::new();
    let mut min = [f32::INFINITY; 3];
    let mut max = [f32::NEG_INFINITY; 3];
    while reader.read_next(&mut timestep)? {
        let seconds = frame_seconds(&timestep, feed)?;
        if let Some(&last) = times.last()
            && seconds <= last
        {
            return Err(format!(
                "frame times must increase; {seconds} s does not follow {last} s in {}",
                feed.path.display()
            )
            .into());
        }
        times.push(seconds);
        for atom in 0..feed.atoms {
            let position = timestep.positions[feed.file_index(atom)];
            min = std::array::from_fn(|axis| min[axis].min(position[axis]));
            max = std::array::from_fn(|axis| max[axis].max(position[axis]));
        }
    }
    if times.is_empty() {
        return Err(format!("{} has no frames", feed.path.display()).into());
    }
    Ok(Scan { times, min, max })
}

/// Maps playback seconds onto one track's physical time: the first frame sits at
/// playback zero, and the last frame at the end of the video.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Clock {
    /// Physical time of the first frame, in seconds.
    pub(crate) first: f64,
    /// Physical time from the first frame to the last, in seconds.
    pub(crate) span: f64,
    /// Length of the video the span is fitted to.
    pub(crate) playback_seconds: f64,
}

impl Clock {
    /// Fits the whole time range of `times` onto `playback_seconds` of video.
    ///
    /// # Errors
    /// Returns an error when there are fewer than two frames.
    pub(crate) fn fit(times: &[f64], playback_seconds: f64) -> Result<Self, Box<dyn Error>> {
        let (Some(&first), Some(&last)) = (times.first(), times.last()) else {
            return Err("the trajectory has no frames".into());
        };
        if times.len() < 2 {
            return Err("a track needs at least two frames to move".into());
        }
        Ok(Self {
            first,
            span: last - first,
            playback_seconds,
        })
    }

    /// The physical time shown at `playback` seconds into the video.
    pub(crate) fn sim_at(self, playback: f64) -> f64 {
        self.first + self.span * (playback / self.playback_seconds)
    }
}

/// One output frame: its index, its playback time, and each track's physical time.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Tick {
    pub(crate) index: u32,
    pub(crate) playback: f64,
    pub(crate) sim: [f64; 2],
}

/// A forward-only cursor over one feed. It holds the two frames playback is between,
/// and reads a new frame only when playback passes the later of them.
pub(crate) struct Lane {
    reader: Box<dyn TrajectoryReader>,
    timestep: Timestep,
    feed: Feed,
    start: TrajectoryFrame,
    end: TrajectoryFrame,
}

impl Lane {
    /// Opens the feed and reads its first two frames.
    ///
    /// # Errors
    /// Returns an error when the file has fewer than two frames or its times do not increase.
    pub(crate) fn open(feed: &Feed) -> Result<Self, Box<dyn Error>> {
        let mut reader = open(feed)?;
        let mut timestep = Timestep::default();
        let start =
            read_frame(&mut *reader, &mut timestep, feed)?.ok_or("the trajectory has no frames")?;
        let end = read_frame(&mut *reader, &mut timestep, feed)?
            .ok_or("the trajectory has one frame; a track needs two")?;
        if end.time_seconds() <= start.time_seconds() {
            return Err("frame times must increase".into());
        }
        Ok(Self {
            reader,
            timestep,
            feed: feed.clone(),
            start,
            end,
        })
    }

    /// The two frames the track is between, for binding or for `set_trajectory_pair`.
    pub(crate) fn pair(&self) -> (TrajectoryFrame, TrajectoryFrame) {
        (self.start.clone(), self.end.clone())
    }

    /// Moves the pair forward until it brackets `seconds`, and returns the time to sample.
    ///
    /// Playback only moves forward, so each frame is read once and the scene shares its
    /// positions by reference count. Past the last frame the time clamps to it.
    ///
    /// # Errors
    /// Returns an error when the file cannot be read or its times stop increasing.
    pub(crate) fn advance_to(&mut self, seconds: f64) -> Result<f64, Box<dyn Error>> {
        while self.end.time_seconds() < seconds {
            let Some(next) = read_frame(&mut *self.reader, &mut self.timestep, &self.feed)? else {
                break;
            };
            if next.time_seconds() <= self.end.time_seconds() {
                return Err(format!(
                    "frame times must increase; frame {} does not follow frame {}",
                    next.index(),
                    self.end.index()
                )
                .into());
            }
            self.start = std::mem::replace(&mut self.end, next);
        }
        Ok(seconds
            .min(self.end.time_seconds())
            .max(self.start.time_seconds()))
    }
}

/// Opens the feed's file and checks that it holds the atoms the feed expects.
fn open(feed: &Feed) -> Result<Box<dyn TrajectoryReader>, Box<dyn Error>> {
    let reader = read_trajectory(&feed.path, &TrajectoryReaderOptions::default())?;
    if reader.n_atoms() != feed.file_atoms {
        return Err(format!(
            "{} holds {} atoms, expected {}",
            feed.path.display(),
            reader.n_atoms(),
            feed.file_atoms
        )
        .into());
    }
    Ok(reader)
}

/// Reads the next frame, mapped onto the structure's atoms, or `None` at the end.
fn read_frame(
    reader: &mut dyn TrajectoryReader,
    timestep: &mut Timestep,
    feed: &Feed,
) -> Result<Option<TrajectoryFrame>, Box<dyn Error>> {
    if !reader.read_next(timestep)? {
        return Ok(None);
    }
    let seconds = frame_seconds(timestep, feed)?;
    let positions: Arc<[[f32; 3]]> = (0..feed.atoms)
        .map(|atom| timestep.positions[feed.file_index(atom)])
        .collect();
    Ok(Some(TrajectoryFrame::new(
        u64::try_from(timestep.frame)?,
        seconds,
        positions,
    )))
}

/// The frame's time in seconds, after checking that it has a time and the file's atoms.
fn frame_seconds(timestep: &Timestep, feed: &Feed) -> Result<f64, Box<dyn Error>> {
    let picoseconds = timestep
        .time
        .filter(|time| time.is_finite())
        .ok_or_else(|| {
            format!(
                "frame {} of {} has no finite time stamp",
                timestep.frame,
                feed.path.display()
            )
        })?;
    if timestep.positions.len() != feed.file_atoms {
        return Err(format!(
            "frame {} of {} holds {} positions, expected {}",
            timestep.frame,
            feed.path.display(),
            timestep.positions.len(),
            feed.file_atoms
        )
        .into());
    }
    Ok(picoseconds * PICOSECOND)
}

#[cfg(test)]
#[path = "track_tests.rs"]
mod tests;
