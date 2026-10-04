//! The verbs the command language accepts.

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
        name: "segment",
        synopsis: "segment JSON | segment style INDEX:GENERATION STYLES_JSON",
        summary: "declare a categorical grid or replace its label styles",
    },
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
        name: "selection",
        synopsis: "selection [QUERY]",
        summary: "replace the current semantic selection, or clear it without a query",
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
        name: "pocket",
        synopsis: "pocket [near=N] [mid=M] [in STRUCTURE], TARGET",
        summary: "draw the pocket-and-pose composition around a target",
    },
    VerbInfo {
        name: "assembly",
        synopsis: "assembly {\"structures\":[...],\"instances\":[...],\"unit_cell\":...} | null",
        summary: "set or remove molecular assembly instances and crystallographic unit-cell guides",
    },
    VerbInfo {
        name: "plane",
        synopsis: "plane {\"structure\":1,\"center\":[...],...}",
        summary: "add a finite caller-authored planar guide",
    },
    VerbInfo {
        name: "volume",
        synopsis: "volume source HASH dims [X,Y,Z] affine [16 values] iso|direct|slice JSON",
        summary: "declare a density volume whose grid arrives through a binding",
    },
    VerbInfo {
        name: "snapshot",
        synopsis: "snapshot save|restore|remove NAME",
        summary: "capture, atomically restore, or remove a named portable scene",
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
