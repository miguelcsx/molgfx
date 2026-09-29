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
        synopsis: "show FORM [control=value]... [duplicate] [as LAYER] [in STRUCTURE] [, TARGET] | show @LAYER",
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
        synopsis: "color COLOR [in STRUCTURE] [, TARGET]",
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
        name: "label",
        synopsis: "label \"TEXT\" [in STRUCTURE], QUERY",
        summary: "add a text label at the centroid of a query",
    },
    VerbInfo {
        name: "distance",
        synopsis: "distance [in STRUCTURE], QUERY, QUERY",
        summary: "measure the distance between two query centroids",
    },
    VerbInfo {
        name: "angle",
        synopsis: "angle [in STRUCTURE], QUERY, QUERY, QUERY",
        summary: "measure the angle at the middle of three query centroids",
    },
    VerbInfo {
        name: "dihedral",
        synopsis: "dihedral [in STRUCTURE], QUERY, QUERY, QUERY, QUERY",
        summary: "measure the torsion about the axis through the middle two of four centroids",
    },
    VerbInfo {
        name: "interaction",
        synopsis: "interaction {\"mode\":\"explicit\",...}",
        summary: "add a caller-supplied explicit overlay interaction",
    },
    VerbInfo {
        name: "focus",
        synopsis: "focus TARGET",
        summary: "highlight and frame a target, muting its distant context",
    },
    VerbInfo {
        name: "auto",
        synopsis: "auto [STRUCTURE]",
        summary: "draw a structure with the size-appropriate default forms",
    },
    VerbInfo {
        name: "volume",
        synopsis: "volume {\"source\":{...},\"dimensions\":[...],...}",
        summary: "declare a density volume whose grid arrives through a binding",
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

/// Colour schemes computed from each atom: element, every category, carbon by
/// chain, and every metric.
#[must_use]
pub fn schemes() -> Vec<&'static str> {
    let mut words = vec!["element", crate::ir::CARBON_BY_CHAIN];
    words.extend(
        molgfx_scene::color::AtomCategory::ALL.map(molgfx_scene::color::AtomCategory::name),
    );
    words.extend(molgfx_scene::color::AtomMetric::ALL.map(molgfx_scene::color::AtomMetric::name));
    words
}

/// Palettes a categorical colour can use.
#[must_use]
pub fn palettes() -> Vec<&'static str> {
    molgfx_scene::color::palette_names()
}

/// Ramps a property colour can use.
/// Every ramp name, each also usable reversed with an `_r` suffix.
#[must_use]
pub fn ramps() -> Vec<&'static str> {
    molgfx_scene::color::ramp_names()
}

/// Named colours, in sRGB.
///
/// A name resolves to one fixed sRGB triple, so a scene that says `color
/// firebrick` reads the same everywhere. The set covers the conventional
/// element and presentation names the reference engines use most; it is
/// deliberately smaller than either engine's full palette table, and every
/// entry here is a colour a caller can also spell as `#rrggbb`.
pub const NAMED_COLORS: &[(&str, [u8; 3])] = &[
    ("black", [0, 0, 0]),
    ("blue", [51, 102, 255]),
    ("brown", [140, 90, 50]),
    ("cyan", [0, 200, 220]),
    ("deep_teal", [0, 120, 130]),
    ("firebrick", [178, 34, 34]),
    ("forest", [34, 139, 34]),
    ("gold", [255, 215, 0]),
    ("gray", [128, 128, 128]),
    ("green", [51, 190, 80]),
    ("grey", [128, 128, 128]),
    ("hot_pink", [255, 105, 180]),
    ("indigo", [75, 0, 130]),
    ("ivory", [255, 255, 240]),
    ("khaki", [240, 230, 140]),
    ("lime", [0, 255, 0]),
    ("magenta", [220, 40, 200]),
    ("marine", [0, 102, 153]),
    ("maroon", [128, 0, 0]),
    ("navy", [0, 0, 128]),
    ("olive", [128, 128, 0]),
    ("orange", [255, 140, 20]),
    ("orchid", [218, 112, 214]),
    ("pink", [255, 150, 190]),
    ("plum", [221, 160, 221]),
    ("purple", [140, 70, 190]),
    ("red", [230, 40, 40]),
    ("salmon", [250, 128, 114]),
    ("sea_green", [46, 139, 87]),
    ("sienna", [160, 82, 45]),
    ("sky_blue", [135, 206, 235]),
    ("slate", [110, 120, 200]),
    ("steel_blue", [70, 130, 180]),
    ("tan", [210, 180, 140]),
    ("teal", [0, 140, 140]),
    ("tomato", [255, 99, 71]),
    ("turquoise", [64, 224, 208]),
    ("violet", [238, 130, 238]),
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
    schemes()
        .into_iter()
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
