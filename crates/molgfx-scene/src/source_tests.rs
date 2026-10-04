use super::{SourceAtom, SourceBond, SourceTopology, topology_identity};

fn topology(order: molframe::BondOrder, metal: bool) -> SourceTopology {
    SourceTopology {
        atoms: vec![
            SourceAtom {
                element: 6,
                residue: 0,
            },
            SourceAtom {
                element: 8,
                residue: 0,
            },
        ]
        .into(),
        residue_atom_start: vec![0, 2].into(),
        chain_residue_start: vec![0, 1].into(),
        model_chain_start: vec![0, 1].into(),
        bonds: vec![SourceBond {
            atoms: [0, 1],
            order,
            aromatic: order == molframe::BondOrder::Aromatic,
            metal,
        }]
        .into(),
        anisotropy: Vec::new().into(),
        secondary_structure: vec![molframe::SecondaryStructure::Unknown].into(),
    }
}

#[test]
fn topology_identity_includes_bond_chemistry() {
    let single = topology(molframe::BondOrder::Single, false);
    let double = topology(molframe::BondOrder::Double, false);
    let metal = topology(molframe::BondOrder::Single, true);

    assert_ne!(topology_identity(&single), topology_identity(&double));
    assert_ne!(topology_identity(&single), topology_identity(&metal));
}

#[test]
fn topology_identity_includes_secondary_structure() {
    let mut helix = topology(molframe::BondOrder::Single, false);
    helix.secondary_structure = vec![molframe::SecondaryStructure::AlphaHelix].into();
    let coil = topology(molframe::BondOrder::Single, false);

    assert_ne!(topology_identity(&helix), topology_identity(&coil));
}

#[test]
fn every_exact_secondary_state_has_a_distinct_topology_identity() {
    use molframe::SecondaryStructure as S;
    let states = [
        S::Unknown,
        S::Coil,
        S::AlphaHelix,
        S::Strand,
        S::Turn,
        S::ThreeTenHelix,
        S::PiHelix,
        S::OtherHelix,
        S::BetaBridge,
        S::Bend,
        S::PolyProline,
    ];
    let mut identities = std::collections::HashSet::new();
    for state in states {
        let mut value = topology(molframe::BondOrder::Single, false);
        value.secondary_structure = vec![state].into();
        assert!(identities.insert(topology_identity(&value)));
    }
}
