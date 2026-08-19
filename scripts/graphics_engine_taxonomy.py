"""Runtime command taxonomy used by the graphics-engine parity audit.

PyMOL exposes application, scripting, chemistry and data-management commands
alongside drawing commands.  They stay in the inventory, but are classified by
ownership so a desktop command is not mistaken for a renderer capability.
"""

from __future__ import annotations


def _names(value: str) -> set[str]:
    return set(value.split())


COMMAND_FAMILIES = {
    "io": _names(
        "fetch load load_coords load_embedded load_mtz load_png load_traj loadall "
        "multifilesave multisave read_mol2str read_pdbstr save write dump"
    ),
    "selection": _names(
        "bychain byobject byres byring bysegment bysegi byss count_atoms "
        "count_discrete deselect extract get_chains get_object_list identify "
        "index indicate remove select flag invert unmask unpick"
    ),
    "query": _names(
        "get get_angle get_area get_bond get_chains get_clip get_dihedral "
        "get_distance get_extent get_position get_renderer get_sasa_relative "
        "get_symmetry get_title get_type get_version get_viewport id_atom"
    ),
    "representation": _names(
        "as callout cartoon cgo curve_new dashes dots ellipsoids hide isodot isomesh "
        "isosurface label mesh nb_spheres nonbonded ribbon show show_as slice "
        "spheres sticks surface trace volume spheroid"
    ),
    "analysis": _names(
        "align angle cealign centerofmass check dss dihedral distance "
        "extra_fit find_pairs fit get_area h_add h_fill hbond intra_fit "
        "intra_rms intra_rms_cur overlap pair_fit pbc_unwrap pbc_wrap "
        "pi_interactions phi_psi reference rms rms_cur super torsion valence "
        "vdw_fit"
    ),
    "camera": _names(
        "center clip focal_blur get_position get_view look_at move orient "
        "origin reset rotate set_view turn view zoom"
    ),
    "presentation": _names(
        "alphatoall assign_stereo bg_color bg_colour color colour color_deep "
        "desaturate gradient ramp_new ramp_update recolor recolour set "
        "set_bond set_color set_colour set_dihedral set_geometry set_key "
        "set_name set_symmetry set_title spectrum stereo unset unset_bond "
        "unset_deep volume_color"
    ),
    "animation": _names(
        "backward count_frames count_states delete_states ending forward frame "
        "join_states mclear mcopy mdelete mdo mdump mem meter_reset middle "
        "minsert mmatrix mmove morph movie.load movie.nutate movie.pause "
        "movie.produce movie.rock movie.roll movie.screw movie.sweep "
        "movie.tdroll movie.zoom mplay mpng mset mstop mtoggle mview rewind "
        "resume rock scene skip split_states"
    ),
    "export": _names("capture draw mpng png ray viewport"),
    "editing": _names(
        "alter alter_state attach bond clean copy copy_to create cycle_valence "
        "delete deprotect editing_ring fab fix_chemistry fnab fragment fuse "
        "h_add h_fix mse2met protect pseudoatom rebond redo remove_picked replace "
        "sculpt_activate sculpt_deactivate sculpt_iterate sculpt_purge smooth "
        "sort split_chains translate unbond undo uniquify"
    ),
    "scripting": _names(
        "alias assert call cd class def del dir do exec for fork global if "
        "import ls pass pop print python pwd raise run space system try while"
    ),
    "scene": _names(
        "disable enable group order rebuild refresh rename scene_order toggle "
        "ungroup"
    ),
    "volume": _names(
        "isolevel map_new map_set map_set_border map_trim slice_new volume_ramp_new"
    ),
    "crystallography": _names("symexp symmetry_copy"),
    "application": _names(
        "abort accept api button cache config_mouse decline diagnostics embed "
        "feedback full_screen help help_setting log log_close log_open quit "
        "refresh_wizard reinitialize replace_wizard spawn splash update window wizard"
    ),
    "ui": _names("cls drag edit edit_mode editing_ring refresh_wizard volume_panel"),
    "utility": _names(
        "util.cbab util.cbac util.cbag util.cbak util.cbam util.cbao util.cbap "
        "util.cbas util.cbaw util.cbay util.cbc util.chainbow util.cnc "
        "util.mrock util.mroll util.rainbow util.ss"
    ),
}


FAMILY_PRIORITY = (
    "io",
    "selection",
    "query",
    "representation",
    "analysis",
    "camera",
    "presentation",
    "animation",
    "export",
    "editing",
    "scripting",
    "scene",
    "volume",
    "crystallography",
    "application",
    "ui",
    "utility",
)


COMMAND_OVERRIDES = {
    "get_area": "analysis",
    "get_sasa_relative": "analysis",
    "get_symmetry": "crystallography",
    "get_position": "camera",
    "get_view": "camera",
    "get_viewport": "camera",
    "get_clip": "camera",
    "scene": "scene",
    "volume": "volume",
    "slice": "volume",
}


NON_RENDERING_FAMILIES = frozenset(
    {"io", "selection", "query", "analysis", "editing", "scripting", "application", "ui", "utility"}
)


EXACT_PDVIEWX_STATUS = {
    "missing": _names("callout cgo ellipsoids spheroid movie.produce capture draw"),
    "partial": _names(
        "mpng movie.load movie.nutate movie.pause movie.rock movie.roll movie.screw "
        "movie.sweep movie.tdroll movie.zoom mpng volume_ramp_new "
        "symexp symmetry_copy"
    ),
}

# These commands have a more specific boundary than their broad family would
# imply. A command-level status describes the closest pdviewx capability; it is
# not a claim that pdviewx exposes a PyMOL-compatible command API.
COMMAND_STATUS_OVERRIDES = {
    "callout": ("partial", "pdviewx-annotations-and-guides"),
    "capture": ("partial", "pdviewx-image-output"),
    "draw": ("partial", "pdviewx-image-output"),
    "movie.produce": ("missing", "caller-owned-movie-timeline-and-encoder"),
    "ellipsoids": ("partial", "pdviewx-analytic-anisotropic-ellipsoid; exact-command-preset-missing"),
    "spheroid": ("partial", "pdviewx-scientific-ellipsoid; exact-preset-missing"),
    "assign_stereo": ("provider-or-caller", "pdbiox-or-chemistry-provider"),
    "get_renderer": ("out-of-scope", "host-backend-query"),
    "stereo": ("out-of-scope", "host-display-integration"),
    "map_new": ("provider-or-caller", "caller-or-provider-map-construction"),
    "map_set": ("provider-or-caller", "caller-or-provider-map-editing"),
    "map_set_border": ("provider-or-caller", "caller-or-provider-map-editing"),
    "map_trim": ("provider-or-caller", "caller-or-provider-map-editing"),
    "map_double": ("provider-or-caller", "caller-or-provider-map-editing"),
    "map_halve": ("provider-or-caller", "caller-or-provider-map-editing"),
    "mappend": ("provider-or-caller", "caller-or-provider-map-editing"),
    "mask": ("provider-or-caller", "caller-or-provider-map-editing"),
    "matrix_copy": ("provider-or-caller", "caller-or-provider-coordinate-editing"),
    "matrix_reset": ("provider-or-caller", "caller-or-provider-coordinate-editing"),
    "matrix_transfer": ("provider-or-caller", "caller-or-provider-coordinate-editing"),
    "gradient": ("partial", "pdviewx-backdrop-profile; exact-gradient-policy-missing"),
    "color_deep": ("partial", "pdviewx-material-depth-policy"),
    "desaturate": ("partial", "pdviewx-display-transform; exact-command-policy-missing"),
    "set_geometry": ("partial", "caller-authored-geometry; no-general-CGO-buffer"),
    "set_key": ("out-of-scope", "application-keyboard-binding"),
    "viewport": ("partial", "pdviewx-image-configuration"),
}


EXECUTED_COMMANDS = _names(
    "alter align angle bg_color cartoon clip color count_atoms count_frames count_states "
    "create dss dihedral disable distance draw enable find_pairs frame get_area get_extent "
    "get_model get_povray get_sasa_relative get_symmetry get_view get_viewport group h_add "
    "hide isodot isomesh isosurface label load_cgo map_new mset mpng mview png pseudoatom "
    "ramp_new read_pdbstr rebuild rms_cur save scene select set set_bond set_color show "
    "show_as slice_new spectrum stereo symexp translate volume volume_ramp_new viewport"
)


def command_execution_evidence(name: str) -> str:
    """Distinguish direct runtime exercise from help-only inventory evidence."""

    return "executed" if name in EXECUTED_COMMANDS else "documented-only"


def pdviewx_mapping(name: str, family: str) -> dict[str, str]:
    """Map a reference command to pdviewx ownership and evidence status."""

    if name in COMMAND_STATUS_OVERRIDES:
        status, owner = COMMAND_STATUS_OVERRIDES[name]
        return {"status": status, "owner": owner}
    if name in EXACT_PDVIEWX_STATUS["missing"]:
        return {"status": "missing", "owner": "pdviewx-or-downstream-exporter"}
    if name in EXACT_PDVIEWX_STATUS["partial"]:
        return {"status": "partial", "owner": "pdviewx-or-downstream-exporter"}
    if family == "selection" or family == "query":
        return {"status": "beta", "owner": "pdviewx-selection"}
    if family == "analysis":
        return {"status": "provider", "owner": "pdbiox-or-caller"}
    if family in {"io", "editing"}:
        return {"status": "provider-or-caller", "owner": "pdbiox-or-caller"}
    if family in {"scripting", "application", "ui", "utility"}:
        return {"status": "out-of-scope", "owner": "application-shell"}
    if family == "animation":
        return {"status": "partial", "owner": "pdviewx-trajectory-or-exporter"}
    if family == "export":
        return {"status": "partial", "owner": "pdviewx-or-downstream-exporter"}
    if family == "scene":
        return {"status": "partial", "owner": "pdviewx-core"}
    if family == "crystallography":
        return {"status": "partial", "owner": "pdbiox-provider-and-pdviewx"}
    if family == "volume":
        return {"status": "beta", "owner": "pdviewx-volume"}
    return {"status": "beta", "owner": "pdviewx-render"}


def classify_command(name: str) -> str:
    """Return an explicit ownership family for a public PyMOL command."""

    if name in COMMAND_OVERRIDES:
        return COMMAND_OVERRIDES[name]
    for family in FAMILY_PRIORITY:
        if name in COMMAND_FAMILIES[family]:
            return family
    if name.startswith("movie.") or name.startswith("m"):
        return "animation"
    if name.startswith("util."):
        return "utility"
    if name.startswith(("get_", "count_", "iterate", "identify", "index")):
        return "query"
    if name.startswith(("load", "read", "save", "write", "fetch")):
        return "io"
    if name.startswith(("set", "unset", "color", "spectrum", "ramp")):
        return "presentation"
    if name.startswith(("get_view", "set_view", "view", "zoom", "orient")):
        return "camera"
    if name.startswith(("get_", "rms", "fit", "align", "super", "find_")):
        return "analysis"
    return "review-needed"
