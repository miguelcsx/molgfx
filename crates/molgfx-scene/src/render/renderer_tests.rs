use crate::{Renderer, Scene, profile, rep};

fn center_pixel(profile: crate::RenderProfile, opacity: f32) -> [u8; 4] {
    let pdb = b"HETATM    1  N   LIG A   1       0.000   0.000  -4.000  1.00 10.00           N\nHETATM    2  O   HOH A   2       0.000   0.000   0.000  1.00 10.00           O\nEND\n";
    let (structure, _) =
        molframe::read_bytes(pdb.to_vec(), Some("two.pdb"), &molframe::ReadOptions::new())
            .expect("fixture");
    let mut scene = Scene::from_structure(&structure).expect("scene");
    scene
        .add(rep::surface("all").opacity(opacity))
        .expect("surface");
    let image = Renderer::with_profile(profile)
        .expect("GPU")
        .render_image(&scene, (96, 96))
        .expect("image");
    let at = (48 * 96 + 48) * 4;
    image.pixels()[at..at + 4].try_into().expect("pixel")
}

#[test]
#[ignore = "requires a native GPU adapter"]
fn a_translucent_surface_differs_from_an_opaque_one_in_every_profile() {
    for profile in [profile::interactive(), profile::converged()] {
        let opaque = center_pixel(profile, 1.0);
        let glass = center_pixel(profile, 0.3);
        assert_ne!(opaque, glass, "opacity 0.3 rendered like 1.0: {glass:?}");
    }
}
