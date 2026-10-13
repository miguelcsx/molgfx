//! The selection words, their accepted aliases and the help for a wrong one.
//!
//! The query language itself belongs to `MolFrame`; this table lists the words
//! a command author is likely to try and maps each to the word `MolFrame`
//! actually defines, so `show lines, ligands` reads as `show lines, ligand`
//! instead of failing on a plural. A word that is neither canonical nor an
//! alias is reported by name with the nearest valid words.

use super::suggest::suggest_many;

/// Group words: each selects a class of atoms and takes no value.
pub const GROUP_WORDS: &[&str] = &[
    "protein",
    "backbone",
    "sidechain",
    "nucleic",
    "nucleicbackbone",
    "nucleicbase",
    "nucleicsugar",
    "water",
    "ion",
    "lipid",
    "saccharide",
    "hetero",
    "hydrogen",
    "heavy",
    "polymer",
    "ligand",
    "aromatic",
    "polar_hydrogen",
    "nonpolar_hydrogen",
    "helix",
    "strand",
    "sheet",
    "alpha_helix",
    "helix_310",
    "pi_helix",
    "polyproline",
    "bridge",
    "turn",
    "bend",
    "coil",
    "all",
    "none",
];

/// Column words: each compares an atom property with a value.
pub const COLUMN_WORDS: &[&str] = &[
    "index",
    "id",
    "chain",
    "label_chain",
    "auth_chain",
    "resid",
    "label_resid",
    "auth_resid",
    "resname",
    "label_resname",
    "auth_resname",
    "name",
    "label_name",
    "auth_name",
    "altloc",
    "model",
    "entity",
    "entity_type",
    "element",
    "segid",
    "icode",
    "record_type",
    "x",
    "y",
    "z",
    "mass",
    "charge",
    "formalcharge",
    "radius",
    "bfactor",
    "occupancy",
    "plddt",
    "pae",
];

/// Alternative spellings and their canonical word, as `(alias, canonical)`.
pub const KEYWORD_ALIASES: &[(&str, &str)] = &[
    ("ligands", "ligand"),
    ("het", "hetero"),
    ("hets", "hetero"),
    ("hetatm", "hetero"),
    ("hetatms", "hetero"),
    ("heteroatom", "hetero"),
    ("heteroatoms", "hetero"),
    ("glycan", "saccharide"),
    ("glycans", "saccharide"),
    ("sugar", "saccharide"),
    ("sugars", "saccharide"),
    ("saccharides", "saccharide"),
    ("ions", "ion"),
    ("solvent", "water"),
    ("waters", "water"),
    ("hoh", "water"),
    ("proteins", "protein"),
    ("lipids", "lipid"),
    ("hydrogens", "hydrogen"),
    ("polymers", "polymer"),
    ("helices", "helix"),
    ("helixes", "helix"),
    ("strands", "strand"),
    ("sheets", "sheet"),
    ("side_chain", "sidechain"),
    ("main_chain", "backbone"),
    ("mainchain", "backbone"),
    ("resi", "resid"),
    ("resno", "resid"),
    ("resnum", "resid"),
    ("resseq", "resid"),
    ("resn", "resname"),
    ("chains", "chain"),
    ("elem", "element"),
    ("atomname", "name"),
    ("atom_name", "name"),
    ("b_factor", "bfactor"),
    ("bfac", "bfactor"),
    ("bfactors", "bfactor"),
    ("temp_factor", "bfactor"),
    ("occ", "occupancy"),
    ("confidence", "plddt"),
];

/// The word `MolFrame` defines for `word`, when `word` is a known alias.
#[must_use]
pub fn canonical_keyword(word: &str) -> Option<&'static str> {
    let lower = word.to_ascii_lowercase();
    KEYWORD_ALIASES
        .iter()
        .find(|(alias, _)| *alias == lower)
        .map(|(_, canonical)| *canonical)
}

/// Up to three valid words nearest to a wrong one, canonical spellings first.
#[must_use]
pub fn nearest_keywords(word: &str) -> Vec<&'static str> {
    let mut found: Vec<&'static str> = Vec::new();
    if let Some(canonical) = canonical_keyword(word) {
        found.push(canonical);
    }
    let canonical = GROUP_WORDS.iter().chain(COLUMN_WORDS).copied();
    let aliases = KEYWORD_ALIASES
        .iter()
        .map(|(alias, _)| *alias)
        .filter(|alias| !alias.is_empty());
    for near in suggest_many(word, canonical.chain(aliases), 6) {
        let canonical = match canonical_keyword(near) {
            Some(canonical) => canonical,
            None => near,
        };
        if !found.contains(&canonical) {
            found.push(canonical);
        }
    }
    found.truncate(3);
    found
}

/// A finding's message with the unknown word named, and the nearest valid
/// words, when the finding is an unknown selection word.
///
/// Returns the message and the first nearest word for the error's suggestion.
#[must_use]
pub fn describe_query_finding(finding: &molframe::Diagnostic) -> (String, Option<String>) {
    let unknown = finding
        .context()
        .iter()
        .find(|item| matches!(item.label(), "keyword" | "property" | "same key"));
    let Some(unknown) = unknown else {
        return (finding.message().to_owned(), None);
    };
    let word = unknown.value();
    let near = nearest_keywords(word);
    let what = match unknown.label() {
        "property" => "numeric property",
        "same key" => "`same` key",
        _ => "selection keyword",
    };
    let message = if near.is_empty() {
        format!(
            "unknown {what} '{word}'; groups such as protein, ligand, ion, water, saccharide and hetero need no value, and columns such as chain, resid, resname, name, element and bfactor compare with one"
        )
    } else {
        format!("unknown {what} '{word}'; did you mean {}?", near.join(", "))
    };
    (message, near.first().map(|first| (*first).to_owned()))
}

#[cfg(test)]
#[path = "keywords_tests.rs"]
mod tests;
