use super::*;

fn same(left: &[f32], right: &[f32]) -> bool {
    left.len() == right.len()
        && left
            .iter()
            .zip(right)
            .all(|(a, b)| a.to_bits() == b.to_bits())
}

#[test]
fn the_heaviest_member_is_dominant_and_the_earliest_wins_a_tie() {
    assert_eq!(dominant_index(&[1.0, 3.0, 2.0]), 1);
    assert_eq!(dominant_index(&[2.0, 2.0]), 0);
    assert_eq!(dominant_index(&[0.0, 0.0, 1.0]), 2);
}

#[test]
fn others_scale_by_their_weight_relative_to_the_dominant_one() {
    let style = EnsembleOpacity::default();
    let Ok(opacities) = ensemble_opacities(&[1.0, 3.0, 0.0], style) else {
        panic!("valid weights");
    };
    assert!(same(
        &opacities,
        &[
            style.alternate_opacity / 3.0,
            style.dominant_opacity,
            style.minimum_opacity
        ]
    ));
}

#[test]
fn invalid_weights_and_opacities_are_rejected() {
    let style = EnsembleOpacity::default();
    assert!(ensemble_opacities(&[], style).is_err());
    assert!(ensemble_opacities(&[0.0, 0.0], style).is_err());
    assert!(ensemble_opacities(&[-1.0, 1.0], style).is_err());
    assert!(ensemble_opacities(&[f32::NAN], style).is_err());
    for bad in [
        EnsembleOpacity {
            minimum_opacity: 0.9,
            ..style
        },
        EnsembleOpacity {
            dominant_opacity: 0.4,
            ..style
        },
        EnsembleOpacity {
            dominant_opacity: 1.5,
            ..style
        },
    ] {
        assert!(ensemble_opacities(&[1.0], bad).is_err());
    }
}
