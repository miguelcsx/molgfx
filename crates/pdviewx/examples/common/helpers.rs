//! Helpers shared by the examples. Not an example itself: Cargo only treats
//! top-level files in `examples/` as build targets.

use pdviewx::SecondaryStructure;

/// Reads a structure, tolerating the local irregularities real archive entries
/// carry, and infers connectivity.
///
/// Deposited `struct_conn` records routinely name atoms that are not in the
/// coordinate list — links to alternate conformers, to hydrogens, to parts left
/// out of the model. Under the default mode one such row invalidates the whole
/// read, which rejects a large share of the archive. `Recover` keeps the entry
/// and reports the affected rows instead.
///
/// # Errors
///
/// Returns an error when the file cannot be read even in recovery mode, or when
/// bond inference fails.
pub fn read_structure(path: &str) -> Result<pdbiox::Structure, String> {
    let options = pdbiox::ReadOptions::new().mode(pdbiox::ParseMode::Recover);
    let (parsed, _diagnostics) = pdbiox::read_with_options(path, &options)
        .map_err(|diagnostics| format!("could not read {path}: {diagnostics:?}"))?;
    let bonded = pdbiox::infer_bonds(
        &parsed,
        pdbiox::BondInference::default(),
        &pdbiox::ExecutionContext::default(),
    )
    .map_err(|diagnostic| format!("bond inference failed for {path}: {diagnostic:?}"))?;
    Ok(bonded.structure)
}

/// Reads the secondary structure an mmCIF file already declares.
///
/// Deposited entries carry `_struct_conf` (helices and turns) and
/// `_struct_sheet_range` (strands). Consuming those is reading the file, not
/// re-deriving the science: assignment from geometry belongs in `pdbiox`, and
/// a scene that ignores the deposited annotation draws every protein as coil.
///
/// Returns an empty vector for files without the categories, which leaves the
/// scene's own coil default in place.
#[must_use]
pub fn deposited_secondary_structure(
    path: &str,
    structure: &pdbiox::Structure,
) -> Vec<(pdbiox::ResidueIndex, SecondaryStructure)> {
    let Ok(bytes) = std::fs::read(path) else {
        return Vec::new();
    };
    let input = pdbiox::InputBuffer::from_bytes(bytes);
    let Ok((document, _diagnostics)) = pdbiox::cif::parse(&input) else {
        return Vec::new();
    };
    let Some(block) = document.first_block() else {
        return Vec::new();
    };

    // (chain label, first residue, last residue) spans, in file order.
    let mut spans: Vec<(String, i32, i32, SecondaryStructure)> = Vec::new();
    collect_spans(block, "struct_conf", None, &mut spans);
    collect_spans(
        block,
        "struct_sheet_range",
        Some(SecondaryStructure::Strand),
        &mut spans,
    );
    if spans.is_empty() {
        return Vec::new();
    }

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

/// Collects one category's residue spans. `forced` overrides the kind for
/// categories that carry no type column of their own.
fn collect_spans(
    block: &pdbiox::cif::DataBlock,
    category_name: &str,
    forced: Option<SecondaryStructure>,
    output: &mut Vec<(String, i32, i32, SecondaryStructure)>,
) {
    let Some(category) = block.category(category_name) else {
        return;
    };
    if category.row_count() == 0 {
        return;
    }
    let mut rows = pdbiox::cif::Rows::new(category);
    loop {
        // Bare mmCIF tokens are identifiers and numbers, not quoted text:
        // reading them as text alone loses every unquoted chain label and
        // sequence number.
        let kind = match forced {
            Some(kind) => Some(kind),
            None => rows
                .identifier("conf_type_id")
                .map(|value| classify(value.as_ref())),
        };
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

/// mmCIF conformation types: `HELX*` are helices, `STRN` strands, `TURN` turns.
fn classify(conf_type: &str) -> SecondaryStructure {
    if conf_type.starts_with("HELX") {
        SecondaryStructure::Helix
    } else if conf_type.starts_with("STRN") {
        SecondaryStructure::Strand
    } else if conf_type.starts_with("TURN") {
        SecondaryStructure::Turn
    } else {
        SecondaryStructure::Coil
    }
}
