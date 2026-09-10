//! Deposited mmCIF secondary structure retained during the browser read.

use pdviewx::SecondaryStructure;

pub(crate) struct ParsedStructure {
    pub(crate) structure: pdbiox::Structure,
    pub(crate) secondary_structure: Vec<(pdbiox::ResidueIndex, SecondaryStructure)>,
}

pub(crate) fn parse_structure(
    bytes: Vec<u8>,
    name: Option<String>,
) -> Result<ParsedStructure, Vec<pdbiox::Diagnostic>> {
    let options = pdbiox::ReadOptions::new();
    let name = name.map(String::into_boxed_str);
    if !looks_like_mmcif(&bytes, name.as_deref()) {
        return pdbiox::read_bytes(bytes, name.as_deref(), &options)
            .and_then(|(structure, _)| finish_structure(&structure, Vec::new()));
    }

    let input = pdbiox::InputBuffer::from_bytes(bytes);
    let (document, structure, _) =
        pdbiox::cif::read_with_metadata(&input, &options, keep_secondary_category)?;
    let secondary_structure = document
        .first_block()
        .map_or_else(Vec::new, |block| deposited_records(block, &structure));
    finish_structure(&structure, secondary_structure)
}

fn finish_structure(
    structure: &pdbiox::Structure,
    secondary_structure: Vec<(pdbiox::ResidueIndex, SecondaryStructure)>,
) -> Result<ParsedStructure, Vec<pdbiox::Diagnostic>> {
    let bonded = pdbiox::infer_bonds(
        structure,
        pdbiox::BondInference::default(),
        &pdbiox::ExecutionContext::default(),
    )
    .map_err(|diagnostic| vec![diagnostic])?;
    Ok(ParsedStructure {
        structure: bonded.structure,
        secondary_structure,
    })
}

fn looks_like_mmcif(bytes: &[u8], name: Option<&str>) -> bool {
    name.is_some_and(|value| {
        std::path::Path::new(value)
            .extension()
            .is_some_and(|extension| {
                extension.eq_ignore_ascii_case("cif") || extension.eq_ignore_ascii_case("mmcif")
            })
    }) || bytes
        .iter()
        .copied()
        .skip_while(u8::is_ascii_whitespace)
        .take(5)
        .eq(b"data_".iter().copied())
}

fn keep_secondary_category(name: &str) -> bool {
    matches!(name, "struct_conf" | "struct_sheet_range")
}

fn deposited_records(
    block: &pdbiox::cif::DataBlock,
    structure: &pdbiox::Structure,
) -> Vec<(pdbiox::ResidueIndex, SecondaryStructure)> {
    let mut spans = Vec::new();
    collect_spans(block, "struct_conf", None, &mut spans);
    collect_spans(
        block,
        "struct_sheet_range",
        Some(SecondaryStructure::Strand),
        &mut spans,
    );

    let mut records = Vec::new();
    for chain in structure.data().chains() {
        let Some(label) = chain.label() else {
            continue;
        };
        for residue in chain.residues() {
            let Some(sequence) = residue.label_seq_id() else {
                continue;
            };
            for (span_label, first, last, kind) in &spans {
                if span_label == label && sequence >= *first && sequence <= *last {
                    records.push((residue.index(), *kind));
                    break;
                }
            }
        }
    }
    records
}

fn collect_spans(
    block: &pdbiox::cif::DataBlock,
    category_name: &str,
    forced: Option<SecondaryStructure>,
    output: &mut Vec<(String, i32, i32, SecondaryStructure)>,
) {
    let Some(category) = block.category(category_name) else {
        return;
    };
    let mut rows = pdbiox::cif::Rows::new(category);
    loop {
        let kind = forced.or_else(|| {
            rows.identifier("conf_type_id")
                .map(|value| classify(value.as_ref()))
        });
        let span = rows
            .identifier("beg_label_asym_id")
            .zip(rows.integer("beg_label_seq_id"))
            .zip(rows.integer("end_label_seq_id"));
        if let (Some(kind), Some(((label, first), last))) = (kind, span)
            && let (Ok(first), Ok(last)) = (i32::try_from(first), i32::try_from(last))
        {
            output.push((label.into_owned(), first, last, kind));
        }
        if !rows.advance() {
            break;
        }
    }
}

fn classify(value: &str) -> SecondaryStructure {
    if value.starts_with("HELX") {
        SecondaryStructure::Helix
    } else if value.starts_with("STRN") {
        SecondaryStructure::Strand
    } else if value.starts_with("TURN") {
        SecondaryStructure::Turn
    } else {
        SecondaryStructure::Coil
    }
}
