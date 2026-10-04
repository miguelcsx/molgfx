//! Source metadata needed to enrich one renderer atom pick.

#[derive(Default)]
pub(super) struct AtomPickDetails {
    pub(super) model: Option<u32>,
    pub(super) entity: Option<u32>,
    pub(super) atom_name: Option<String>,
    pub(super) auth_atom_name: Option<String>,
    pub(super) altloc: Option<String>,
    pub(super) element: Option<u8>,
    pub(super) residue_index: Option<u32>,
    pub(super) residue_name: Option<String>,
    pub(super) auth_residue_name: Option<String>,
    pub(super) chain: Option<String>,
    pub(super) auth_chain: Option<String>,
    pub(super) residue_number: Option<i32>,
    pub(super) insertion_code: Option<String>,
    pub(super) occupancy: Option<f32>,
    pub(super) b_factor: Option<f32>,
    pub(super) formal_charge: Option<i8>,
    pub(super) position: Option<[f32; 3]>,
    pub(super) secondary_structure: Option<String>,
    pub(super) label: String,
}

impl AtomPickDetails {
    fn unresolved(atom_index: u32) -> Self {
        Self {
            label: format!("atom {atom_index}"),
            ..Self::default()
        }
    }
}

pub(super) fn atom_pick_details(
    source: Option<&molframe::Structure>,
    atom_index: u32,
) -> AtomPickDetails {
    let Some(source) = source else {
        return AtomPickDetails::unresolved(atom_index);
    };

    let engine = source.engine();

    let Some(atom) = engine.atom(molframe::AtomIndex::new(atom_index)) else {
        return AtomPickDetails::unresolved(atom_index);
    };

    let atom_name_ref = atom.name();
    let atom_name = atom_name_ref.map(str::to_owned);
    let auth_atom_name = atom.auth_name().map(str::to_owned);
    let altloc = atom.alt_label().map(str::to_owned);
    let element = atom.element().map(molframe::Element::atomic_number);
    let occupancy = atom.occupancy();
    let b_factor = atom.b_factor();
    let formal_charge = atom.formal_charge();
    let model = (engine.model_count() > 0).then_some(0);
    let position = atom.position();

    let Some(residue) = atom.residue() else {
        let label = match atom_name_ref {
            Some(name) => name.to_owned(),
            None => format!("atom {atom_index}"),
        };

        return AtomPickDetails {
            model,
            atom_name,
            auth_atom_name,
            altloc,
            element,
            occupancy,
            b_factor,
            formal_charge,
            position,
            label,
            ..AtomPickDetails::default()
        };
    };

    let residue_index = residue.index().get();
    let residue_name_ref = residue.name();
    let residue_name = residue_name_ref.map(str::to_owned);
    let auth_residue_name = residue.auth_name().map(str::to_owned);
    let residue_number = residue.auth_seq_id().or_else(|| residue.label_seq_id());
    let insertion_code = residue.ins_code().map(str::to_owned);

    let (chain, auth_chain, entity) = engine
        .data()
        .topology
        .chains
        .containing(residue_index)
        .and_then(|chain_index| engine.chain(chain_index))
        .map_or((None, None, None), |chain| {
            (
                chain.label().map(str::to_owned),
                chain.auth_label().map(str::to_owned),
                chain.entity().map(molframe::EntityIndex::get),
            )
        });

    let secondary_structure = usize::try_from(residue_index)
        .ok()
        .and_then(|index| engine.secondary_structure().get(index))
        .map(|value| secondary_structure_name(*value).to_owned());

    let atom_label = atom_name_ref.map_or("atom", |name| name);
    let residue_label = residue_name_ref.map_or("?", |name| name);
    let chain_label = chain.as_deref().map_or("?", |name| name);
    let label = match residue_number {
        Some(number) => format!("{atom_label} {residue_label}{number} {chain_label}"),
        None => format!("{atom_label} {residue_label} {chain_label}"),
    };

    AtomPickDetails {
        model,
        entity,
        atom_name,
        auth_atom_name,
        altloc,
        element,
        residue_index: Some(residue_index),
        residue_name,
        auth_residue_name,
        chain,
        auth_chain,
        residue_number,
        insertion_code,
        occupancy,
        b_factor,
        formal_charge,
        position,
        secondary_structure,
        label,
    }
}

fn secondary_structure_name(value: molframe::SecondaryStructure) -> &'static str {
    molgfx_core::SecondaryStructure::from(value).name()
}
