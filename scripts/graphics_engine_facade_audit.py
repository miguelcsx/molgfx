#!/usr/bin/env python3
"""Audit the graphics capability contract at the public ``molgfx`` facade.

This is a source/API audit, not a language benchmark.  It checks that the
public types needed by the reference-engine capability families are actually
reachable from the facade, while retaining explicit negative records for
features that the parity ledger says are missing or caller-owned.
"""

from __future__ import annotations

import argparse
import json
import re
from collections import Counter
from pathlib import Path
from typing import Any

try:
    from graphics_engine_reference_manifest import build_feature_coverage
except ModuleNotFoundError:
    from scripts.graphics_engine_reference_manifest import build_feature_coverage


CAPABILITY_ANCHORS: tuple[dict[str, Any], ...] = (
    {
        "id": "molecular-representations",
        "references": ["PyMOL", "VMD", "ChimeraX", "OVITO", "Protein Imager"],
        "symbols": [
            "Representation",
            "RepresentationKind",
            "RepresentationParams",
            "ParticleShape",
            "SurfaceKind",
            "Material",
        ],
        "evidence": ["crates/molgfx-core/src/representation", "scripts/graphics_engine_native_probe.py"],
    },
    {
        "id": "backbone-and-special-structure",
        "references": ["VMD", "PyMOL", "ChimeraX", "Protein Imager"],
        "symbols": [
            "SecondaryStructure",
            "TubeRadiusMapping",
            "CarbohydrateSymbol",
            "AnisotropicEllipsoid",
        ],
        "evidence": ["crates/molgfx-geometry/src/cartoon", "crates/molgfx-core/src/structure"],
    },
    {
        "id": "surface-and-volume-rendering",
        "references": ["PyMOL", "VMD", "ChimeraX", "OVITO", "Protein Imager"],
        "symbols": [
            "DensityVolume",
            "VolumeRendering",
            "VolumeSlice",
            "VolumeRegion",
            "VolumeTransferFunction",
            "SurfaceScalarOverlay",
            "SegmentedVolume",
        ],
        "evidence": ["crates/molgfx-core/src/structure/density.rs", "crates/molgfx-render/src/engine/volume_tests.rs"],
    },
    {
        "id": "camera-clipping-and-optics",
        "references": ["PyMOL", "VMD", "ChimeraX", "OVITO", "Protein Imager"],
        "symbols": [
            "Camera",
            "Projection",
            "ClipSet",
            "RenderProfile",
            "LightingEnvironment",
            "BackdropStyle",
            "DisplayTransform",
            "DepthOfField",
        ],
        "evidence": ["crates/molgfx-render/src/engine", "crates/molgfx-core/src/selection/clipping.rs"],
    },
    {
        "id": "labels-measurements-and-interactions",
        "references": ["PyMOL", "VMD", "ChimeraX", "OVITO", "Protein Imager"],
        "symbols": [
            "Annotation",
            "Measurement",
            "Guide",
            "InteractionEdge",
            "ValidationMarker",
            "Pick",
            "PickEntity",
        ],
        "evidence": ["crates/molgfx-core/src/representation/annotation.rs", "crates/molgfx-render/src/engine/picking.rs"],
    },
    {
        "id": "trajectory-ensembles-and-primitives",
        "references": ["VMD", "PyMOL", "ChimeraX", "OVITO"],
        "symbols": [
            "TrajectoryFrame",
            "TrajectorySegment",
            "Ensemble",
            "Particle",
            "ParticleMotion",
            "Primitive",
            "PrimitiveHandle",
            "ParticleShape",
            "MeshInstance",
            "MeshInstanceHandle",
            "ScreenOverlay",
            "OverlayHandle",
            "OverlayContent",
        ],
        "evidence": ["crates/molgfx-core/src/structure/trajectory.rs", "crates/molgfx-render/src/passes"],
    },
    {
        "id": "semantic-focus-and-comparison",
        "references": ["Mol*", "ChimeraX", "Protein Imager"],
        "symbols": [
            "FocusScene",
            "DifferenceView",
            "EnsembleView",
            "LodPolicy",
            "StreamPlanner",
        ],
        "evidence": ["crates/molgfx-semantic/src", "docs/PARITY.md"],
    },
    {
        "id": "scene-session-and-image-output",
        "references": ["PyMOL", "ChimeraX", "Mol*", "OVITO"],
        "symbols": [
            "Scene",
            "SceneManifest",
            "SceneDescription",
            "SceneDescriptionSources",
            "RenderSession",
            "Image",
            "ImageConfig",
            "RenderMode",
            "FrameOutcome",
            "RenderError",
        ],
        "evidence": ["crates/molgfx-core/src/serialization", "crates/molgfx-render/src/engine"],
    },
    {
        "id": "host-target-and-capability-seam",
        "references": ["VMD", "ChimeraX", "OVITO", "Protein Imager"],
        "symbols": ["Capabilities", "WindowSource", "WindowTarget", "Engine"],
        "evidence": ["crates/molgfx/src/lib.rs", "crates/molgfx-gpu/src/device.rs"],
    },
)

NEGATIVE_CAPABILITIES: tuple[dict[str, Any], ...] = (
    {
        "id": "external-renderer-selection",
        "references": ["VMD", "OVITO", "YASARA"],
        "status": "out-of-scope",
        "reason": "External ray/render backend selection belongs to a caller or application shell.",
    },
    {
        "id": "movie-encoder",
        "references": ["PyMOL", "VMD", "ChimeraX", "OVITO", "YASARA"],
        "status": "out-of-scope",
        "reason": "The engine returns PNG/frame state; timeline and codec ownership stay downstream.",
    },
    {
        "id": "python-binding",
        "references": ["PyMOL", "VMD", "OVITO"],
        "status": "out-of-scope",
        "reason": "Bindings are a downstream facade over the stable Rust library.",
    },
)


def _split_use_items(value: str) -> list[str]:
    """Split a Rust use-group without splitting nested braces."""

    items: list[str] = []
    depth = 0
    start = 0
    for index, character in enumerate(value):
        if character == "{":
            depth += 1
        elif character == "}":
            depth -= 1
        elif character == "," and depth == 0:
            items.append(value[start:index])
            start = index + 1
    items.append(value[start:])
    return items


def public_facade_symbols(path: Path) -> set[str]:
    """Extract names re-exported by the facade without inspecting internals."""

    source = path.read_text(encoding="utf-8")
    symbols: set[str] = set(re.findall(r"\bpub\s+(?:type|struct|enum|trait)\s+([A-Za-z_][A-Za-z0-9_]*)", source))
    for match in re.finditer(r"\bpub\s+use\s+([^;]+);", source, re.DOTALL):
        statement = re.sub(r"//[^\n]*", "", match.group(1)).strip()
        group = re.search(r"\{(.*)\}", statement, re.DOTALL)
        if group:
            items = _split_use_items(group.group(1))
        else:
            items = [statement.rsplit("::", 1)[-1]]
        for item in items:
            item = item.strip()
            if not item or item == "self":
                continue
            item = item.split(" as ", 1)[-1].strip()
            name = re.match(r"[A-Za-z_][A-Za-z0-9_]*", item)
            if name:
                symbols.add(name.group(0))
    return symbols


def audit(repo: Path) -> dict[str, Any]:
    facade_path = repo / "crates/molgfx/src/lib.rs"
    symbols = public_facade_symbols(facade_path)
    anchors = []
    for capability in CAPABILITY_ANCHORS:
        missing = [name for name in capability["symbols"] if name not in symbols]
        anchors.append(
            {
                **capability,
                "status": "passed" if not missing else "missing-facade-symbols",
                "missing_symbols": missing,
            }
        )

    static_records = build_feature_coverage({})
    status_counts = Counter(record["status"] for record in static_records)
    source_counts = Counter(record["source"] for record in static_records)
    negative = []
    for capability in NEGATIVE_CAPABILITIES:
        if capability["id"] == "python-binding":
            present = (repo / "crates/molgfx-py").exists() or (repo / "python/molgfx").exists()
            negative.append({**capability, "status": "present-unexpectedly" if present else capability["status"]})
        else:
            negative.append({**capability, "status": capability["status"]})

    missing_anchor_symbols = sorted(
        {symbol for anchor in anchors for symbol in anchor["missing_symbols"]}
    )
    return {
        "schema": 1,
        "status": "passed" if not missing_anchor_symbols else "failed",
        "scope": "public molgfx graphics capability facade; pdbiox remains provider-only",
        "facade": str(facade_path),
        "facade_symbol_count": len(symbols),
        "facade_symbols": sorted(symbols),
        "capability_anchors": anchors,
        "negative_capabilities": negative,
        "static_manifest": {
            "records": len(static_records),
            "by_source": dict(sorted(source_counts.items())),
            "by_status": dict(sorted(status_counts.items())),
            "unreviewed": [record for record in static_records if record["status"] == "unreviewed"],
        },
        "missing_facade_symbols": missing_anchor_symbols,
    }


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--repo", type=Path, default=Path(__file__).resolve().parents[1])
    parser.add_argument("--output", type=Path)
    parser.add_argument("--check", action="store_true")
    args = parser.parse_args()
    result = audit(args.repo.resolve())
    rendered = json.dumps(result, indent=2) + "\n"
    if args.output:
        args.output.parent.mkdir(parents=True, exist_ok=True)
        args.output.write_text(rendered, encoding="utf-8")
    print(rendered, end="")
    return 0 if result["status"] == "passed" or not args.check else 1


if __name__ == "__main__":
    raise SystemExit(main())
