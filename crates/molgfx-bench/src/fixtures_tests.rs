use super::{named, scene, scenes_root};
use std::path::{Path, PathBuf};

#[test]
fn a_declared_name_resolves_inside_the_corpus() {
    let resolved = scene(Some(Path::new("3PTB.cif")));
    assert_eq!(resolved, Some(scenes_root().join("3PTB.cif")));
}

#[test]
fn a_declared_name_without_an_extension_gains_the_cif_one() {
    assert_eq!(named(Path::new("1ubq")), PathBuf::from("1ubq.cif"));
}

#[test]
fn a_declared_non_cif_name_is_never_renamed() {
    assert_eq!(
        scene(Some(Path::new("2DT3-hexasaccharide.pdb"))),
        Some(scenes_root().join("2DT3-hexasaccharide.pdb"))
    );
}

#[test]
fn a_typed_path_is_taken_as_written() {
    let typed = Path::new("/data/set/pose.pdb");
    assert_eq!(scene(Some(typed)), Some(typed.to_path_buf()));
}

#[test]
fn no_request_resolves_to_nothing() {
    assert_eq!(scene(None), None);
}
