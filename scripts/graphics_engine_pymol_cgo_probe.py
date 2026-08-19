"""Exercise every public PyMOL CGO constant in a disposable runtime."""

from __future__ import annotations

from pathlib import Path
from typing import Any

from graphics_engine_cross_render import read_png
from graphics_engine_reference_manifest import PYMOL_CGO_DISPOSITIONS, PYMOL_CGO_OPCODES


CGO_ROLES = {
    "ALPHA": "state",
    "ALPHA_TRIANGLE": "primitive",
    "BEGIN": "stream-structure",
    "BEZIER": "primitive",
    "CHAR": "text",
    "COLOR": "state",
    "CONE": "primitive",
    "CUSTOM_CYLINDER": "primitive",
    "CYLINDER": "primitive",
    "DISABLE": "state",
    "DOTWIDTH": "state",
    "ELLIPSOID": "primitive",
    "ENABLE": "state",
    "END": "stream-structure",
    "FONT": "text-state",
    "FONT_AXES": "text-state",
    "FONT_SCALE": "text-state",
    "FONT_VERTEX": "text-state",
    "LIGHTING": "state-argument",
    "LINES": "begin-mode",
    "LINEWIDTH": "state",
    "LINE_LOOP": "begin-mode",
    "LINE_STRIP": "begin-mode",
    "NORMAL": "stream-attribute",
    "NULL": "stream-sentinel",
    "PICK_COLOR": "picking-state",
    "POINTS": "begin-mode",
    "QUADRIC": "primitive",
    "SAUSAGE": "primitive",
    "SPHERE": "primitive",
    "STOP": "stream-sentinel",
    "TRIANGLE": "primitive",
    "TRIANGLES": "begin-mode",
    "TRIANGLE_FAN": "begin-mode",
    "TRIANGLE_STRIP": "begin-mode",
    "VERTEX": "stream-attribute",
    "WIDTHSCALE": "state",
}


def _triangle(cgo: Any) -> list[float]:
    """Return the documented TRIANGLE record with vertices, normals and colors."""

    return [
        cgo.TRIANGLE,
        0.0,
        0.0,
        0.0,
        1.0,
        0.0,
        0.0,
        0.0,
        1.0,
        0.0,
        0.0,
        0.0,
        1.0,
        0.0,
        0.0,
        1.0,
        0.0,
        0.0,
        0.0,
        1.0,
        0.0,
        0.0,
        0.0,
        1.0,
        0.0,
        0.0,
        1.0,
        0.0,
        0.0,
    ]


def _triangle_stream(cgo: Any, mode: float) -> list[float]:
    """Build a visible BEGIN/END stream for one OpenGL primitive mode."""

    vertices = [
        cgo.VERTEX,
        0.0,
        0.0,
        0.0,
        cgo.VERTEX,
        1.0,
        0.0,
        0.0,
        cgo.VERTEX,
        0.0,
        1.0,
        0.0,
    ]
    if mode in (cgo.POINTS, cgo.LINES):
        vertices.extend([cgo.VERTEX, 1.0, 1.0, 0.0])
    return [
        cgo.BEGIN,
        mode,
        cgo.COLOR,
        0.2,
        0.8,
        1.0,
        cgo.NORMAL,
        0.0,
        0.0,
        1.0,
        *vertices,
        cgo.END,
        cgo.COLOR,
        1.0,
        0.3,
        0.2,
        cgo.SPHERE,
        0.0,
        0.0,
        0.5,
        0.35,
    ]


def _font_stream(cgo: Any) -> list[float]:
    """Use the low-level font records and a sphere fallback for visibility."""

    return [
        cgo.FONT,
        12.0,
        1.0,
        1.0,
        cgo.FONT_SCALE,
        1.0,
        1.0,
        cgo.FONT_VERTEX,
        -1.0,
        -1.0,
        0.0,
        cgo.FONT_AXES,
        1.0,
        0.0,
        0.0,
        0.0,
        1.0,
        0.0,
        0.0,
        0.0,
        1.0,
        cgo.COLOR,
        1.0,
        1.0,
        1.0,
        cgo.CHAR,
        65.0,
        cgo.SPHERE,
        0.0,
        0.0,
        0.5,
        0.35,
    ]


def _stream_for(opcode: str, cgo: Any) -> tuple[list[float], str]:
    """Return a bounded valid stream and explain how the record is exercised."""

    sphere = [cgo.COLOR, 1.0, 0.3, 0.2, cgo.SPHERE, 0.0, 0.0, 0.5, 0.35]
    if opcode in {"POINTS", "LINES", "LINE_LOOP", "LINE_STRIP", "TRIANGLES", "TRIANGLE_FAN", "TRIANGLE_STRIP"}:
        return _triangle_stream(cgo, getattr(cgo, opcode)), "inside BEGIN/END primitive-mode stream"
    if opcode == "BEGIN":
        return _triangle_stream(cgo, cgo.TRIANGLES), "stream opener with triangle mode"
    if opcode == "END":
        return _triangle_stream(cgo, cgo.TRIANGLES), "stream terminator with triangle mode"
    if opcode in {"VERTEX", "NORMAL"}:
        return _triangle_stream(cgo, cgo.TRIANGLES), "per-vertex stream attribute"
    if opcode == "STOP":
        return sphere + [cgo.STOP], "terminator after visible primitive"
    if opcode == "NULL":
        return [cgo.NULL, *sphere], "no-op sentinel before visible primitive"
    if opcode == "COLOR":
        return [cgo.COLOR, 0.2, 0.8, 1.0, *sphere[0:1], *sphere[1:]], "RGB state before primitive"
    if opcode == "ALPHA":
        return [cgo.ALPHA, 0.6, *sphere], "alpha state before primitive"
    if opcode in {"LINEWIDTH", "WIDTHSCALE", "DOTWIDTH"}:
        return [getattr(cgo, opcode), 2.0, *sphere], "line/point state before primitive"
    if opcode in {"ENABLE", "DISABLE"}:
        return [getattr(cgo, opcode), cgo.LIGHTING, *sphere], "lighting state before primitive"
    if opcode == "LIGHTING":
        return [cgo.ENABLE, cgo.LIGHTING, *sphere], "lighting capability argument to ENABLE"
    if opcode == "PICK_COLOR":
        return [cgo.PICK_COLOR, 1.0, -4.0, *sphere], "atom/bond picking state before primitive"
    if opcode in {"FONT", "FONT_SCALE", "FONT_VERTEX", "FONT_AXES", "CHAR"}:
        return _font_stream(cgo), "low-level text stream with visible primitive fallback"
    if opcode == "SPHERE":
        return sphere, "sphere primitive"
    if opcode == "TRIANGLE":
        return _triangle(cgo) + sphere, "standalone triangle primitive"
    if opcode == "ALPHA_TRIANGLE":
        vertices = [0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0, 0.0]
        normals = [0.0, 0.0, 1.0] * 3
        colors = [1.0, 0.0, 0.0, 0.6, 0.0, 1.0, 0.0, 0.6, 0.0, 0.0, 1.0, 0.6]
        return [
            cgo.ALPHA_TRIANGLE,
            0.0,
            1.0 / 3.0,
            1.0 / 3.0,
            0.0,
            0.0,
            *vertices,
            *normals,
            *colors,
        ] + sphere, "alpha triangle with three RGBA vertices"
    if opcode == "CYLINDER":
        return [cgo.CYLINDER, -1.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.2, 1.0, 0.0, 0.0, 0.0, 1.0, 0.0], "gradient cylinder primitive"
    if opcode == "CONE":
        return [cgo.CONE, -1.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.35, 0.1, 1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 1.0, 1.0], "capped cone primitive"
    if opcode == "SAUSAGE":
        return [cgo.SAUSAGE, -1.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.2, 1.0, 0.0, 0.0, 0.0, 1.0, 0.0], "spherocylinder primitive"
    if opcode == "CUSTOM_CYLINDER":
        return [cgo.CUSTOM_CYLINDER, -1.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.2, 1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0], "custom-cylinder primitive"
    if opcode == "ELLIPSOID":
        return [cgo.ELLIPSOID, 0.0, 0.0, 0.5, 0.35, 1.0, 0.0, 0.0, 0.0, 0.8, 0.0, 0.0, 0.0, 1.2], "anisotropic ellipsoid primitive"
    if opcode == "QUADRIC":
        return [cgo.QUADRIC, 0.0, 0.0, 0.5, 0.35, 1.0, 0.0, 0.0, 1.0, 0.0, 0.0, 1.0, 0.0, 0.0, 1.0] + sphere, "quadric record plus visible fallback"
    if opcode == "BEZIER":
        return [cgo.BEZIER, -1.0, 0.0, 0.0, -0.3, 1.0, 0.0, 0.3, 1.0, 0.0, 1.0, 0.0, 0.0] + sphere, "cubic Bezier primitive"
    raise KeyError(opcode)


def cgo_opcode_matrix(cmd: Any, root: Path) -> dict[str, Any]:
    """Load, count and ray-render one safe fixture for every public CGO record."""

    from pymol import cgo

    runtime_opcodes = sorted(
        name for name in dir(cgo) if name.isupper() and name not in {"DEFAULT_ERROR", "DEFAULT_SUCCESS"}
    )
    if runtime_opcodes != sorted(PYMOL_CGO_OPCODES):
        raise RuntimeError(
            f"public CGO manifest drift: runtime={runtime_opcodes!r} manifest={sorted(PYMOL_CGO_OPCODES)!r}"
        )
    records: list[dict[str, Any]] = []
    for opcode in PYMOL_CGO_OPCODES:
        name = f"probe_cgo_opcode_{opcode.lower()}"
        record: dict[str, Any] = {
            "opcode": opcode,
            "numeric": float(getattr(cgo, opcode)),
            "role": CGO_ROLES[opcode],
            "pdviewx": {"status": PYMOL_CGO_DISPOSITIONS[opcode][0], "reason": PYMOL_CGO_DISPOSITIONS[opcode][1]},
        }
        try:
            stream, fixture = _stream_for(opcode, cgo)
            cmd.delete(name)
            cmd.load_cgo(stream, name)
            states = int(cmd.count_states(name))
            cmd.disable("all")
            cmd.enable(name)
            cmd.zoom(name, 1.5)
            image = root / f"pymol-cgo-opcode-{opcode.lower()}.png"
            cmd.png(str(image), width=64, height=64, ray=1, quiet=1)
            width, height, pixels = read_png(image)
            signal = sum(1 for red, green, blue, _alpha in pixels if max(red, green, blue) > 4)
            record.update(
                {
                    "status": "passed",
                    "fixture": fixture,
                    "stream_length": len(stream),
                    "object_loaded": name in cmd.get_names("objects"),
                    "states": states,
                    "image": {"width": width, "height": height, "signal_pixels": signal},
                }
            )
        except Exception as error:
            record.update({"status": "failed", "error": f"{type(error).__name__}: {error}"[:500]})
        records.append(record)
    status_counts: dict[str, int] = {}
    for record in records:
        status = str(record["status"])
        status_counts[status] = status_counts.get(status, 0) + 1
    return {
        "manifest_opcode_count": len(PYMOL_CGO_OPCODES),
        "runtime_opcode_count": len(runtime_opcodes),
        "tested_opcode_count": len(records),
        "all_public_opcodes_tested": len(records) == len(PYMOL_CGO_OPCODES),
        "status_counts": dict(sorted(status_counts.items())),
        "records": records,
    }
