"""Probe PyMOL's graphics-relevant module-only API surface.

``cmd.keyword`` is the command-shell registry, not the complete Python API.
This probe keeps the documented graphics/export/session callbacks visible and
gives every module-only callable a runtime disposition: executed, failed,
external-input, contract-only, safety-skipped, or internal. It does not treat
an unexercised callback or state-mutator as a passing feature.
"""

from __future__ import annotations

import contextlib
import io
import inspect
import re
from pathlib import Path
from typing import Any, Callable


GRAPHICS_MODULE_APIS = (
    "copy_image",
    "get_collada",
    "get_gltf",
    "get_idtf",
    "get_mtl_obj",
    "get_movie_length",
    "get_movie_locked",
    "get_movie_playing",
    "get_object_ttt",
    "get_povray",
    "get_scene_list",
    "get_scene_thumbnail",
    "get_session",
    "get_vis",
    "get_volume_field",
    "get_volume_histogram",
    "get_vrml",
    "load_callback",
    "load_cgo",
    "load_map",
    "load_model",
    "load_object",
    "load_raw",
    "move_on_curve",
    "set_object_ttt",
    "set_session",
    "set_vis",
)


MODULE_GRAPHICS_EXACT = {
    "copy_image", "get_collada", "get_gltf", "get_idtf", "get_mtl_obj",
    "get_color_index", "get_color_indices", "get_color_tuple", "get_colorection",
    "get_movie_length", "get_movie_locked", "get_movie_playing", "get_object_ttt",
    "get_povray", "get_scene_list", "get_scene_message", "get_scene_thumbnail",
    "get_session", "get_vis", "get_volume_field", "get_volume_histogram", "get_vrml",
    "ipython_image", "label2", "load_callback", "load_cgo", "load_map", "move_on_curve",
    "publication", "set_colorection", "set_object_ttt", "set_session", "set_vis",
}

MODULE_ANALYSIS_EXACT = {
    "alter_list", "fast_minimize", "load_brick", "minimize", "select_list",
    "set_discrete", "translate_atom",
}

MODULE_PROVIDER_PREFIXES = (
    "get_atom_", "get_bond", "get_cif", "get_coord", "get_fast", "get_phipsi",
    "get_pdb", "get_raw_alignment", "get_state", "load_coord", "load_model",
    "load_object", "load_raw", "read_", "write", "save", "download_chem_comp",
)
MODULE_SCENE_PREFIXES = (
    "get_object_", "get_names", "get_scene_", "get_setting_", "get_title",
    "get_view", "set_object_", "set_scene_", "set_state_", "set_vis",
)
MODULE_ANIMATION_PREFIXES = (
    "get_frame", "get_movie", "move_on_curve", "set_frame", "mclear", "mcopy",
    "mdelete", "mdo", "mdump", "minsert", "mmove", "mplay", "mset", "mstop",
    "mview",
)
MODULE_APP_PREFIXES = (
    "button", "complete", "dirty_wizard", "edit", "editor", "fb_", "gui", "help",
    "keyboard", "lock", "paste", "reaper", "show_help", "wizard",
)
MODULE_SCRIPTING_NAMES = {
    "do", "exec", "extend", "extendaa", "fork", "python", "run", "system",
}


MODULE_RUNTIME_EXTERNAL = {
    "download_chem_comp": "requires network or a local chemical-component database",
    "load_brick": "requires an external brick/volume fixture",
    "map_generate": "requires an MTZ/reflection-file fixture",
    "read_xplorstr": "requires an XPLOR/CNS volume fixture",
}

MODULE_RUNTIME_CONTRACT = {
    "move_on_curve": "requires a curve object and caller-owned motion contract",
    "set_session": "replaces the complete engine state; session round-trip is probed separately",
}

MODULE_RUNTIME_INTERNAL = {
    "get_colorection": "undocumented color-correction helper is not a public feature contract",
    "set_colorection": "undocumented color-correction helper is not a public feature contract",
}

MODULE_RUNTIME_UNAVAILABLE = {
    "alter_list": "PyMOL documents this module entry as an unsupported feature",
    "fast_minimize": "PyMOL documents this module entry as an unsupported nonfunctional command",
}

MODULE_RUNTIME_SPECIAL = {
    "add_bond",
    "alter_list",
    "auto_measure",
    "copy_image",
    "find_pairs",
    "finish_object",
    "get_assembly_ids",
    "get_atom_coords",
    "get_bond_print",
    "get_bonds",
    "get_cifstr",
    "get_color_index",
    "get_color_tuple",
    "get_coords",
    "get_coordset",
    "get_fastastr",
    "get_model",
    "get_names",
    "get_object_matrix",
    "get_object_settings",
    "get_pdbstr",
    "get_raw_alignment",
    "get_scene_message",
    "get_scene_thumbnail",
    "get_selection_state",
    "get_setting_boolean",
    "get_setting_float",
    "get_setting_int",
    "get_setting_legacy",
    "get_setting_text",
    "get_setting_tuple",
    "get_volume_field",
    "get_volume_histogram",
    "label2",
    "ipython_image",
    "load_callback",
    "load_cgo",
    "load_coords",
    "load_coordset",
    "load_map",
    "load_model",
    "load_object",
    "load_raw",
    "minimize",
    "publication",
    "read_molstr",
    "read_mmodstr",
    "read_pdbstr",
    "read_sdfstr",
    "select_list",
    "set_discrete",
    "set_frame",
    "set_object_color",
    "set_object_ttt",
    "set_raw_alignment",
    "set_scene_message",
    "set_state_order",
    "set_vis",
    "transform_object",
    "transform_selection",
    "translate_atom",
    "write_html_ref",
}


MODULE_MOL = """module-mol
  PyMOL

  3  2  0  0  0  0  0  0  0  0999 V2000
    0.0000    0.0000    0.0000 C   0  0  0  0  0  0  0  0  0  0  0  0
    1.4000    0.0000    0.0000 C   0  0  0  0  0  0  0  0  0  0  0  0
    2.1000    1.2000    0.0000 O   0  0  0  0  0  0  0  0  0  0  0  0
  1  2  1  0  0  0  0
  2  3  1  0  0  0  0
M  END
"""


MODULE_SDF = MODULE_MOL + "$$$$\n"


MODULE_PDB = """ATOM      1  N   ALA A   1       0.000   0.000   0.000  1.00 10.00           N
ATOM      2  CA  ALA A   1       1.450   0.000   0.000  1.00 10.00           C
ATOM      3  C   ALA A   1       2.000   1.300   0.000  1.00 10.00           C
ATOM      4  O   ALA A   1       1.300   2.250   0.000  1.00 10.00           O
TER
END
"""


def _summary(value: Any) -> dict[str, Any]:
    """Keep large export/session/array results bounded and JSON-friendly."""

    if isinstance(value, str):
        return {"type": "str", "length": len(value)}
    if isinstance(value, (bytes, bytearray)):
        return {"type": type(value).__name__, "length": len(value)}
    if isinstance(value, dict):
        return {"type": "dict", "length": len(value)}
    if isinstance(value, (list, tuple)):
        return {"type": type(value).__name__, "length": len(value)}
    shape = getattr(value, "shape", None)
    if shape is not None:
        return {"type": type(value).__name__, "shape": list(shape)}
    return {"type": type(value).__name__, "repr": repr(value)[:160]}


def module_callable_mapping(name: str) -> tuple[str, str, str]:
    """Assign every module-only callable an explicit owner and status."""

    if name in MODULE_GRAPHICS_EXACT or name.startswith(("get_volume", "load_cgo")):
        return "graphics-or-scene", "partial", "pdviewx-render-or-downstream-exporter"
    if name in MODULE_ANALYSIS_EXACT:
        return "provider-or-analysis", "provider-or-caller", "pdbiox-or-caller"
    if name in MODULE_SCRIPTING_NAMES or name.startswith(MODULE_APP_PREFIXES):
        return "application-or-scripting", "out-of-scope", "application-shell"
    if name.startswith(MODULE_ANIMATION_PREFIXES):
        return "animation-scene", "partial", "pdviewx-trajectory-or-caller-exporter"
    if name.startswith(MODULE_SCENE_PREFIXES):
        return "scene-state", "partial", "pdviewx-core-or-caller"
    if name.startswith(MODULE_PROVIDER_PREFIXES):
        return "provider-or-caller", "provider-or-caller", "pdbiox-or-caller"
    if re.search(r"(bond|coord|model|object|selection|assembly|chem|map|density|surface|mesh|phipsi|measure|pair|align|fit|rms|sasa|symmetr|state)", name, re.IGNORECASE):
        return "provider-or-analysis", "provider-or-caller", "pdbiox-or-caller"
    return "internal-runtime", "out-of-scope", "PyMOL-runtime-internal"


def module_callable_record(name: str, function: Any) -> dict[str, Any]:
    """Describe one callable without invoking it."""

    family, status, owner = module_callable_mapping(name)
    try:
        signature = str(inspect.signature(function))
    except (TypeError, ValueError):
        signature = "<unavailable>"
    documentation = inspect.getdoc(function) or ""
    return {
        "name": name,
        "family": family,
        "status": status,
        "owner": owner,
        "signature": signature[:500],
        "doc": re.sub(r"\s+", " ", documentation)[:500],
    }


def _module_matrix() -> list[float]:
    """Return a finite identity matrix in PyMOL's flat 4x4 representation."""

    return [
        1.0,
        0.0,
        0.0,
        0.0,
        0.0,
        1.0,
        0.0,
        0.0,
        0.0,
        0.0,
        1.0,
        0.0,
        0.0,
        0.0,
        0.0,
        1.0,
    ]


def _module_special_call(cmd: Any, name: str, root: Path) -> Callable[[], Any] | None:
    """Return a bounded call for APIs whose parameter names are ambiguous."""

    class Callback:
        def __call__(self, *args: Any) -> None:
            return None

    def set_raw_alignment() -> Any:
        cmd.align(
            "probe_alignment_copy",
            "probe_alignment",
            object="module_alignment",
            cycles=0,
            quiet=1,
        )
        raw = cmd.get_raw_alignment("module_alignment")
        cmd.delete("module_alignment")
        return cmd.set_raw_alignment("module_alignment_new", raw)

    calls: dict[str, Callable[[], Any]] = {
        "add_bond": lambda: cmd.add_bond("ala", 1, 2),
        "alter_list": lambda: cmd.alter_list("ala", ["b=42.0"] * 4),
        "auto_measure": lambda: cmd.auto_measure(),
        "copy_image": lambda: cmd.copy_image(),
        "find_pairs": lambda: cmd.find_pairs("ala and name N", "ala and name CA"),
        "finish_object": lambda: cmd.finish_object("probe_object"),
        "get_assembly_ids": lambda: cmd.get_assembly_ids("probe_crystal"),
        "get_atom_coords": lambda: cmd.get_atom_coords("ala and name CA"),
        "get_bond_print": lambda: cmd.get_bond_print("ala", 1, 1),
        "get_bonds": lambda: cmd.get_bonds("ala"),
        "get_cifstr": lambda: cmd.get_cifstr("ala"),
        "get_color_index": lambda: cmd.get_color_index("red"),
        "get_color_tuple": lambda: cmd.get_color_tuple("red"),
        "get_coords": lambda: cmd.get_coords("ala"),
        "get_coordset": lambda: cmd.get_coordset("probe_object"),
        "get_fastastr": lambda: cmd.get_fastastr("ala"),
        "get_model": lambda: cmd.get_model("ala"),
        "get_names": lambda: cmd.get_names(),
        "get_object_matrix": lambda: cmd.get_object_matrix("probe_object"),
        "get_object_settings": lambda: cmd.get_object_settings("probe_object"),
        "get_pdbstr": lambda: cmd.get_pdbstr("ala"),
        "get_raw_alignment": lambda: (
            cmd.align(
                "probe_alignment_copy",
                "probe_alignment",
                object="module_alignment",
                cycles=0,
                quiet=1,
            ),
            cmd.get_raw_alignment("module_alignment"),
        )[1],
        "get_scene_message": lambda: cmd.get_scene_message("probe_scene"),
        "get_scene_thumbnail": lambda: cmd.get_scene_thumbnail("probe_scene"),
        "get_selection_state": lambda: cmd.get_selection_state("ala"),
        "get_setting_boolean": lambda: cmd.get_setting_boolean("orthoscopic"),
        "get_setting_float": lambda: cmd.get_setting_float("orthoscopic"),
        "get_setting_int": lambda: cmd.get_setting_int("orthoscopic"),
        "get_setting_legacy": lambda: cmd.get_setting_legacy("orthoscopic"),
        "get_setting_text": lambda: cmd.get_setting_text("orthoscopic"),
        "get_setting_tuple": lambda: cmd.get_setting_tuple("orthoscopic"),
        "get_volume_field": lambda: cmd.get_volume_field("probe_density"),
        "get_volume_histogram": lambda: cmd.get_volume_histogram("probe_density"),
        "label2": lambda: cmd.label2("ala and name CA", "name"),
        "ipython_image": lambda: cmd.ipython_image(width=32, height=32, ray=1, quiet=1),
        "load_callback": lambda: cmd.load_callback(Callback(), "module_callback", 1, 1, 0),
        "load_cgo": lambda: cmd.load_cgo([], "module_cgo"),
        "load_coords": lambda: cmd.load_coords(cmd.get_coords("ala"), "ala"),
        "load_coordset": lambda: cmd.load_coordset(
            cmd.get_coordset("ala"), "probe_object", 1
        ),
        "load_map": lambda: cmd.load_map(str(root / "probe-density.mrc"), "module_map"),
        "load_model": lambda: cmd.load_model(cmd.get_model("ala"), "module_model"),
        "load_object": lambda: cmd.load_object(1, cmd.get_model("ala"), "module_object"),
        "load_raw": lambda: cmd.load_raw(MODULE_PDB, "pdb", "module_raw"),
        "minimize": lambda: cmd.minimize("ala", iter=1),
        "publication": lambda: cmd.publication("ala"),
        "read_molstr": lambda: cmd.read_molstr(MODULE_MOL, "module_mol"),
        "read_mmodstr": lambda: cmd.read_mmodstr(MODULE_PDB, "module_mmod"),
        "read_pdbstr": lambda: cmd.read_pdbstr(MODULE_PDB, "module_pdb"),
        "read_sdfstr": lambda: cmd.read_sdfstr(MODULE_SDF, "module_sdf"),
        "select_list": lambda: cmd.select_list("module_selection", "ala", [1, 2, 3, 4]),
        "set_discrete": lambda: cmd.set_discrete("probe_object", 1),
        "set_frame": lambda: cmd.set_frame(1),
        "set_object_color": lambda: cmd.set_object_color("probe_object", "red"),
        "set_object_ttt": lambda: cmd.set_object_ttt("probe_object", _module_matrix()),
        "set_raw_alignment": set_raw_alignment,
        "set_scene_message": lambda: cmd.set_scene_message("probe_scene", "module probe"),
        "set_state_order": lambda: cmd.set_state_order("probe_ensemble", [1, 2]),
        "set_vis": lambda: cmd.set_vis(cmd.get_vis()),
        "transform_object": lambda: cmd.transform_object("probe_object", _module_matrix()),
        "transform_selection": lambda: cmd.transform_selection("ala", _module_matrix()),
        "translate_atom": lambda: cmd.translate_atom("ala and name CA", 0.1, 0.0, 0.0),
        "write_html_ref": lambda: cmd.write_html_ref(str(root / "module-reference.html")),
    }
    return calls.get(name)


def _module_generic_value(parameter: inspect.Parameter, root: Path) -> Any:
    """Build a conservative value for a required module-callable parameter."""

    name = parameter.name.lower()
    if name in {"selection", "selection1", "selection2", "sele", "sele1", "sele2"}:
        return "ala"
    if name in {"object", "object_name", "oname", "name", "target", "mobile", "source"}:
        return "probe_object" if name not in {"mobile", "source"} else "ala"
    if name in {"objname", "obj", "map", "map_name", "source_map"}:
        return "probe_density"
    if name in {"color", "colour"}:
        return "red"
    if name in {"dict"}:
        return {}
    if name in {"key", "format", "mode", "type"}:
        return "pdb" if name == "format" else 1
    if name in {"file", "filename", "fname", "path"}:
        return str(root / "fixture.pdb")
    if name in {"content", "contents", "molstr", "sdfstr"}:
        return MODULE_SDF if name == "sdfstr" else MODULE_PDB
    if name in {"expr", "expression", "command"}:
        return "b=42.0" if name != "command" else "count_atoms ala"
    if name in {"matrix", "ttt"}:
        return _module_matrix()
    if name in {"coords", "coordset"}:
        return [[0.0, 0.0, 0.0]]
    if name in {"id_list", "order", "raw"}:
        return [1, 2, 3, 4]
    if name in {"index", "index1", "index2", "id", "state", "state1", "state2"}:
        return 1 if name not in {"index2"} else 2
    if name in {"v0", "v1", "v2", "x", "y", "z"}:
        return 0.1
    if name in {"quiet", "log", "zoom", "finish", "discrete", "copy"}:
        return 1
    if parameter.annotation is int:
        return 1
    if parameter.annotation is float:
        return 1.0
    return "probe"


def _module_required_arguments(function: Any, root: Path) -> tuple[list[Any] | None, str | None]:
    """Resolve only required positional parameters; defaults remain engine-owned."""

    try:
        signature = inspect.signature(function)
    except (TypeError, ValueError):
        return None, "signature unavailable"
    arguments: list[Any] = []
    for parameter in signature.parameters.values():
        if parameter.name in {"_self", "self"} or parameter.default is not inspect.Parameter.empty:
            continue
        if parameter.kind in {inspect.Parameter.VAR_POSITIONAL, inspect.Parameter.VAR_KEYWORD}:
            return None, "variadic argument contract requires caller-specific values"
        if parameter.kind == inspect.Parameter.KEYWORD_ONLY:
            return None, "required keyword-only contract requires caller-specific values"
        arguments.append(_module_generic_value(parameter, root))
    return arguments, None


def _module_runtime_record(
    cmd: Any,
    name: str,
    function: Any,
    family: str,
    root: Path,
) -> dict[str, Any]:
    """Execute one safe module call or record why execution is not meaningful."""

    if family == "internal-runtime":
        return {
            "status": "internal-not-invoked",
            "reason": "PyMOL runtime helper rather than a documented public feature",
        }
    if family == "application-or-scripting":
        return {
            "status": "safety-skipped",
            "reason": "application, GUI, lock, or arbitrary-command surface",
        }
    if name in MODULE_RUNTIME_EXTERNAL:
        return {"status": "external-input", "reason": MODULE_RUNTIME_EXTERNAL[name]}
    if name in MODULE_RUNTIME_UNAVAILABLE:
        return {"status": "runtime-feature-unavailable", "reason": MODULE_RUNTIME_UNAVAILABLE[name]}
    if name in MODULE_RUNTIME_INTERNAL:
        return {"status": "runtime-internal", "reason": MODULE_RUNTIME_INTERNAL[name]}
    if name in MODULE_RUNTIME_CONTRACT:
        return {"status": "contract-only", "reason": MODULE_RUNTIME_CONTRACT[name]}

    call = _module_special_call(cmd, name, root)
    if call is None and name in MODULE_RUNTIME_SPECIAL:
        return {
            "status": "contract-only",
            "reason": "selected API has no bounded fixture call in this runtime",
        }
    arguments: list[Any] | None = []
    reason: str | None = None
    if call is None:
        arguments, reason = _module_required_arguments(function, root)
        if arguments is None:
            return {"status": "contract-only", "reason": reason}

        def invoke() -> Any:
            return function(*arguments)

    else:

        def invoke() -> Any:
            return call()

    try:
        from graphics_engine_pymol_command_audit import prepare_fixture

        with contextlib.redirect_stdout(io.StringIO()), contextlib.redirect_stderr(io.StringIO()):
            prepare_fixture(cmd, root)
            result = invoke()
        return {
            "status": "executed-passed",
            "arguments": [repr(value)[:160] for value in arguments],
            "result": _summary(result),
        }
    except Exception as error:
        return {
            "status": "executed-failed",
            "arguments": [repr(value)[:160] for value in arguments],
            "error": f"{type(error).__name__}: {error}"[:500],
        }


def module_graphics_api_surface(cmd: Any, root: Path) -> dict[str, Any]:
    """Inventory and disposition every public module-only callable."""

    public = {name for name in dir(cmd) if not name.startswith("_")}
    keyword = set(cmd.keyword)
    callable_names = {name for name in public if callable(getattr(cmd, name, None))}
    module_only = sorted(callable_names - keyword)
    module_inventory = [
        module_callable_record(name, getattr(cmd, name)) for name in module_only
    ]
    runtime_by_name: dict[str, dict[str, Any]] = {}
    for item in module_inventory:
        runtime = _module_runtime_record(
            cmd, item["name"], getattr(cmd, item["name"]), item["family"], root
        )
        item["runtime"] = runtime
        runtime_by_name[item["name"]] = runtime

    results: dict[str, dict[str, Any]] = {}
    for name in GRAPHICS_MODULE_APIS:
        function = getattr(cmd, name, None)
        runtime = runtime_by_name.get(name)
        if not callable(function):
            results[name] = {"callable": False, "status": "missing"}
        elif runtime is None:
            results[name] = {
                "callable": True,
                "status": "not-inventory",
                "reason": "callable is not module-only in this runtime",
            }
        else:
            results[name] = {"callable": True, "runtime": runtime, "status": runtime["status"]}

    runtime_statuses = [item["runtime"]["status"] for item in module_inventory]
    return {
        "public_attribute_count": len(public),
        "public_callable_count": len(callable_names),
        "module_only_callable_count": len(module_only),
        "module_only_inventory": module_inventory,
        "module_only_family_counts": {
            family: sum(item["family"] == family for item in module_inventory)
            for family in sorted({item["family"] for item in module_inventory})
        },
        "module_only_status_counts": {
            status: sum(item["status"] == status for item in module_inventory)
            for status in sorted({item["status"] for item in module_inventory})
        },
        "module_runtime_status_counts": {
            status: runtime_statuses.count(status) for status in sorted(set(runtime_statuses))
        },
        "module_only_internal_runtime": [
            item["name"] for item in module_inventory if item["family"] == "internal-runtime"
        ],
        "module_only_mapping_complete": len(module_inventory) == len(module_only),
        "module_runtime_probe_complete": len(runtime_by_name) == len(module_only),
        "graphics_api_count": len(GRAPHICS_MODULE_APIS),
        "graphics_apis": results,
    }
