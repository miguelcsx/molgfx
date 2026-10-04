use super::{CategoryPalette, MAX_PALETTE_COLORS};

#[test]
fn every_palette_fits_the_fixed_colour_block_and_is_never_empty() {
    for palette in CategoryPalette::ALL {
        assert!(palette.len() >= 2, "{palette:?}");
        assert!(palette.len() <= MAX_PALETTE_COLORS, "{palette:?}");
    }
}

#[test]
fn names_are_unique_and_round_trip() {
    for palette in CategoryPalette::ALL {
        assert_eq!(CategoryPalette::from_name(palette.name()), Some(palette));
    }
    assert_eq!(CategoryPalette::from_name("nope"), None);
}

#[test]
fn a_category_past_the_end_cycles_and_a_non_category_has_no_colour() {
    let palette = CategoryPalette::Dark2;
    let first = palette.color(0.0);
    assert!(first.is_some());
    assert_eq!(palette.color(8.0), first);
    assert_eq!(palette.color(17.0), palette.color(1.0));
    assert_eq!(palette.color(-1.0), None);
    assert_eq!(palette.color(1.5), None);
    assert_eq!(palette.color(f32::NAN), None);
}

#[test]
fn the_default_palette_keeps_the_established_eight_hues() {
    assert_eq!(CategoryPalette::default(), CategoryPalette::CvdSafe);
    assert_eq!(CategoryPalette::CvdSafe.len(), 8);
}

#[test]
fn secondary_states_remain_visually_distinct_in_the_categorical_palette() {
    let palette = CategoryPalette::SecondaryStructure;
    for (index, first) in crate::SecondaryStructure::ALL.into_iter().enumerate() {
        let first_colour = palette
            .color(f32::from(first.code()))
            .expect("state colour");
        for second in &crate::SecondaryStructure::ALL[index + 1..] {
            assert_ne!(
                Some(first_colour),
                palette.color(f32::from(second.code())),
                "{first:?} and {second:?} must not collapse to one colour"
            );
        }
    }
}
