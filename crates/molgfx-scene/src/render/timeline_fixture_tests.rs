//! A two-track scene shared by the timeline sequence tests.

use crate::{Scene, StructureId, TrajectoryFrame, rep};
use molgfx_core::Timeline;
use std::sync::Arc;

/// A structure whose trajectory morphs over 0 to 4 ns across one second.
pub(super) fn morphing_scene() -> (Scene, Timeline) {
    let pdb = b"HETATM    1  N   LIG A   1       0.000   0.000  -4.000  1.00 10.00           N\nHETATM    2  O   HOH A   2       0.000   0.000   0.000  1.00 10.00           O\nEND\n";
    let (structure, _) =
        molframe::read_bytes(pdb.to_vec(), Some("two.pdb"), &molframe::ReadOptions::new())
            .unwrap_or_else(|error| panic!("{error}"));
    let mut scene = Scene::from_structure(&structure).unwrap_or_else(|error| panic!("{error}"));
    scene
        .add(rep::spacefill(crate::sel::all()))
        .unwrap_or_else(|error| panic!("{error}"));
    let id = StructureId::new(1);
    let from = TrajectoryFrame::new(0, 0.0, Arc::from(vec![[0.0f32; 3]; 2]));
    let to = TrajectoryFrame::new(1, 4.0e-9, Arc::from(vec![[3.0f32; 3]; 2]));
    let mut timeline = Timeline::new();
    scene
        .morph_timeline_trajectory(&mut timeline, id, from, to, 1.0)
        .unwrap_or_else(|error| panic!("{error}"));
    (scene, timeline)
}
