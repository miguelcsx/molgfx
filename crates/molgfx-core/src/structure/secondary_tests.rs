use super::SecondaryStructure;

const NATIVE: [molframe::SecondaryStructure; 11] = [
    molframe::SecondaryStructure::Unknown,
    molframe::SecondaryStructure::Coil,
    molframe::SecondaryStructure::AlphaHelix,
    molframe::SecondaryStructure::Strand,
    molframe::SecondaryStructure::Turn,
    molframe::SecondaryStructure::ThreeTenHelix,
    molframe::SecondaryStructure::PiHelix,
    molframe::SecondaryStructure::OtherHelix,
    molframe::SecondaryStructure::BetaBridge,
    molframe::SecondaryStructure::Bend,
    molframe::SecondaryStructure::PolyProline,
];

#[test]
fn every_native_state_preserves_its_code_label_and_profile_family() {
    for (native, state) in NATIVE.into_iter().zip(SecondaryStructure::ALL) {
        assert_eq!(SecondaryStructure::from(native), state);
        assert_eq!(state.code(), native.code());
        assert_eq!(SecondaryStructure::from_code(native.code()), Some(state));
        assert_eq!(SecondaryStructure::from_name(state.name()), Some(state));
        assert_eq!(state.is_helix(), native.is_helix());
        assert_eq!(state.is_strand(), native.is_strand());
        assert_eq!(state.is_sheet_like(), native.is_sheet_like());
    }
    assert_eq!(SecondaryStructure::default(), SecondaryStructure::Unknown);
}

#[test]
fn malformed_codes_and_obsolete_labels_are_not_unknown_assignments() {
    for code in 11..=u8::MAX {
        assert_eq!(SecondaryStructure::from_code(code), None);
    }
    for name in ["helix", "bridge", "AlphaHelix", "poly_proline", ""] {
        assert_eq!(SecondaryStructure::from_name(name), None);
    }
}
