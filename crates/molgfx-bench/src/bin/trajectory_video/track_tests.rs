use super::{Feed, frame_seconds};
use molframe::trajectory::Timestep;
use std::path::PathBuf;

fn feed(file_atoms: usize) -> Feed {
    Feed {
        path: PathBuf::from("sample.xtc"),
        map: None,
        file_atoms,
        atoms: file_atoms,
    }
}

fn step(time: Option<f64>, atoms: usize) -> Timestep {
    Timestep {
        time,
        positions: vec![[0.0; 3]; atoms],
        ..Timestep::default()
    }
}

#[test]
fn a_frame_time_in_picoseconds_is_returned_in_seconds() {
    let seconds = frame_seconds(&step(Some(250.0), 2), &feed(2)).expect("time");
    assert!((seconds - 2.5e-10).abs() < 1.0e-22, "{seconds}");
}

#[test]
fn a_frame_without_a_time_is_refused() {
    assert!(frame_seconds(&step(None, 2), &feed(2)).is_err());
}

#[test]
fn a_nan_frame_time_is_refused() {
    assert!(frame_seconds(&step(Some(f64::NAN), 2), &feed(2)).is_err());
}

#[test]
fn a_frame_with_another_atom_count_than_the_feed_is_refused() {
    assert!(frame_seconds(&step(Some(1.0), 3), &feed(2)).is_err());
}
