//! The command vocabulary, listed for completion, help and suggestions.
//!
//! Everything the parser accepts by name is declared here once: the verbs, the
//! forms and their controls (through [`FormKind`]), the colour schemes, the
//! named colours and the property ramps. A user interface lists exactly what
//! the parser accepts, and a misspelling is matched against the same table.

mod suggest;

pub use suggest::suggest;

use crate::ir::FormKind;
use serde::Serialize;

/// One command verb.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize)]
pub struct VerbInfo {
    /// The verb.
    pub name: &'static str,
    /// How the command is written.
    pub synopsis: &'static str,
    /// What it does.
    pub summary: &'static str,
}

/// Every verb, in the order a help listing shows them.
pub const VERBS: &[VerbInfo] = &[
    VerbInfo {
        name: "select",
        synopsis: "select NAME, QUERY",
        summary: "define or redefine a named selection; everything using it follows",
    },
    VerbInfo {
        name: "unselect",
        synopsis: "unselect NAME",
        summary: "remove a named selection nothing uses",
    },
    VerbInfo {
        name: "show",
        synopsis: "show FORM [control=value]... [duplicate] [as LAYER] [in STRUCTURE], TARGET | show @LAYER",
        summary: "draw a target with a form, reusing an identical layer, or reveal a layer",
    },
    VerbInfo {
        name: "hide",
        synopsis: "hide @LAYER",
        summary: "hide a layer without removing it",
    },
    VerbInfo {
        name: "remove",
        synopsis: "remove @LAYER",
        summary: "remove a layer",
    },
    VerbInfo {
        name: "color",
        synopsis: "color COLOR [in STRUCTURE], TARGET",
        summary: "colour a layer, or the atoms of a query in every layer",
    },
    VerbInfo {
        name: "uncolor",
        synopsis: "uncolor [in STRUCTURE] [, QUERY]",
        summary: "remove colour rules",
    },
    VerbInfo {
        name: "opacity",
        synopsis: "opacity VALUE, @LAYER",
        summary: "set a layer's opacity between 0 and 1",
    },
    VerbInfo {
        name: "focus",
        synopsis: "focus TARGET",
        summary: "highlight and frame a target, muting its distant context",
    },
    VerbInfo {
        name: "unfocus",
        synopsis: "unfocus",
        summary: "clear the focus",
    },
    VerbInfo {
        name: "undo",
        synopsis: "undo",
        summary: "undo the last edit",
    },
    VerbInfo {
        name: "redo",
        synopsis: "redo",
        summary: "redo the last undone edit",
    },
];

/// Colour schemes computed from each atom.
pub const SCHEMES: &[&str] = &["element", "chain", "residue", "secondary_structure"];

/// Ramps a property colour can use.
pub const RAMPS: &[&str] = &["viridis", "plasma", "coolwarm"];

/// Named colours, in sRGB.
pub const NAMED_COLORS: &[(&str, [u8; 3])] = &[
    ("black", [0, 0, 0]),
    ("blue", [51, 102, 255]),
    ("brown", [140, 90, 50]),
    ("cyan", [0, 200, 220]),
    ("gray", [128, 128, 128]),
    ("green", [51, 190, 80]),
    ("grey", [128, 128, 128]),
    ("magenta", [220, 40, 200]),
    ("orange", [255, 140, 20]),
    ("pink", [255, 150, 190]),
    ("purple", [140, 70, 190]),
    ("red", [230, 40, 40]),
    ("salmon", [250, 128, 114]),
    ("slate", [110, 120, 200]),
    ("teal", [0, 140, 140]),
    ("wheat", [245, 222, 179]),
    ("white", [255, 255, 255]),
    ("yellow", [250, 220, 40]),
];

/// The sRGB value of a named colour.
#[must_use]
pub fn named_color(name: &str) -> Option<[u8; 3]> {
    NAMED_COLORS
        .iter()
        .find(|(known, _)| *known == name)
        .map(|(_, rgb)| *rgb)
}

/// Every word that names a colour: schemes first, then named colours.
pub fn color_words() -> impl Iterator<Item = &'static str> {
    SCHEMES
        .iter()
        .copied()
        .chain(NAMED_COLORS.iter().map(|(name, _)| *name))
}

/// Every verb name.
pub fn verb_names() -> impl Iterator<Item = &'static str> {
    VERBS.iter().map(|verb| verb.name)
}

/// Every form name.
pub fn form_names() -> impl Iterator<Item = &'static str> {
    FormKind::ALL.iter().map(|kind| kind.name())
}
