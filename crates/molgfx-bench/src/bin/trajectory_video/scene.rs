//! The scene the video renders: both structures, their trajectories, and the clock
//! that maps each output frame onto every track's physical time.

use std::error::Error;

use molgfx::schema::DataSource;
use molgfx::{Scene, StructureId, TrajectoryBinding, color, rep, sel};

use crate::system::Layout;
use crate::track::{Clock, Lane, Scan, Tick};

/// Empty space between the two tracks, in ångström.
const GAP: f32 = 8.0;

/// The scene, its two tracks, and what is needed to move them frame by frame.
pub(crate) struct Stage {
    pub(crate) scene: Scene,
    ids: [StructureId; 2],
    lanes: [Lane; 2],
    clocks: [Clock; 2],
}

impl Stage {
    /// Adds both structures with their trajectories, bound to each track's first two frames.
    ///
    /// # Errors
    /// Returns an error when a file cannot be opened or the scene rejects a track.
    pub(crate) fn build(
        layout: &Layout,
        scans: [&Scan; 2],
        placements: [[f32; 16]; 2],
        seconds: f64,
    ) -> Result<Self, Box<dyn Error>> {
        let mut scene = Scene::empty();
        let left = scene.add_structure_at(&layout.structures[0], placements[0])?;
        scene.add(
            rep::cartoon(sel::all())
                .color(color::secondary_structure())
                .structure(left),
        )?;
        let right = scene.add_structure_at(&layout.structures[1], placements[1])?;
        scene.add(
            rep::ball_and_stick(sel::all())
                .color(color::element())
                .structure(right),
        )?;
        let ids = [left, right];

        let lanes = [Lane::open(&layout.feeds[0])?, Lane::open(&layout.feeds[1])?];
        let clocks = [
            Clock::fit(&scans[0].times, seconds)?,
            Clock::fit(&scans[1].times, seconds)?,
        ];
        for (((id, lane), scan), clock) in ids.iter().zip(&lanes).zip(scans).zip(clocks) {
            let source = DataSource::new(format!("trajectory-{id:?}"));
            let frames = u64::try_from(scan.times.len())?;
            scene.add(molgfx::trajectory::trajectory(*id, source.clone(), frames))?;
            let (start, end) = lane.pair();
            let binding =
                TrajectoryBinding::new(source, start, end).sample_seconds(clock.sim_at(0.0));
            scene.bind_trajectory(binding)?;
        }
        Ok(Self {
            scene,
            ids,
            lanes,
            clocks,
        })
    }

    /// The clocks that map playback onto each track's physical time.
    pub(crate) fn clocks(&self) -> [Clock; 2] {
        self.clocks
    }

    /// The output frames: frame `i` shows playback time `i / fps`.
    pub(crate) fn ticks(&self, frames: u32, fps: u32) -> Vec<Tick> {
        (0..frames)
            .map(|index| {
                let playback = f64::from(index) / f64::from(fps);
                Tick {
                    index,
                    playback,
                    sim: self.clocks.map(|clock| clock.sim_at(playback)),
                }
            })
            .collect()
    }

    /// Moves both tracks to the physical times of `tick`, reading only the frames playback has passed.
    ///
    /// # Errors
    /// Returns an error when a frame cannot be read or the scene rejects a pair.
    pub(crate) fn seek(&mut self, tick: &Tick) -> Result<(), Box<dyn Error>> {
        let Self {
            scene, ids, lanes, ..
        } = self;
        for ((lane, id), sim) in lanes.iter_mut().zip(ids.iter()).zip(tick.sim) {
            let seconds = lane.advance_to(sim)?;
            let (start, end) = lane.pair();
            scene.set_trajectory_pair(*id, start, end, seconds)?;
        }
        Ok(())
    }
}

/// Places the two tracks side by side, each centred on its own box in y and z, with
/// `GAP` between them.
pub(crate) fn placements(scans: [&Scan; 2]) -> [[f32; 16]; 2] {
    let widths = scans.map(|scan| scan.max[0] - scan.min[0]);
    let left_x = -(GAP * 0.5 + widths[0] * 0.5);
    let right_x = GAP * 0.5 + widths[1] * 0.5;
    [
        translation(left_x, scans[0]),
        translation(right_x, scans[1]),
    ]
}

/// A column-major transform that moves `scan`'s box centre to (`target_x`, 0, 0).
fn translation(target_x: f32, scan: &Scan) -> [f32; 16] {
    let centre: [f32; 3] =
        std::array::from_fn(|axis| f32::midpoint(scan.min[axis], scan.max[axis]));
    let mut matrix = [0.0; 16];
    for diagonal in [0, 5, 10, 15] {
        matrix[diagonal] = 1.0;
    }
    matrix[12] = target_x - centre[0];
    matrix[13] = -centre[1];
    matrix[14] = -centre[2];
    matrix
}

/// The box that holds both tracks over every frame, in world coordinates.
pub(crate) fn framing(scans: [&Scan; 2], placements: [[f32; 16]; 2]) -> ([f32; 3], [f32; 3]) {
    let min = std::array::from_fn(|axis| {
        scans
            .iter()
            .zip(placements)
            .map(|(scan, placement)| scan.min[axis] + placement[12 + axis])
            .fold(f32::INFINITY, f32::min)
    });
    let max = std::array::from_fn(|axis| {
        scans
            .iter()
            .zip(placements)
            .map(|(scan, placement)| scan.max[axis] + placement[12 + axis])
            .fold(f32::NEG_INFINITY, f32::max)
    });
    (min, max)
}
