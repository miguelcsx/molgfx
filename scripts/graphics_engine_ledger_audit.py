#!/usr/bin/env python3
"""Check that the documented gap table mirrors the static feature manifest.

The source inventory proves that the reviewed vocabulary is complete.  This
audit proves the user-facing ledger also names every static ``missing`` and
``partial`` record, so a newly added reference feature cannot disappear between
the machine-readable manifest and the documentation.
"""

from __future__ import annotations

import argparse
import json
import re
from collections import defaultdict
from pathlib import Path
from typing import Any

try:
    from graphics_engine_reference_manifest import (
        CHIMERAX_IMAGE_COMMANDS,
        CHIMERAX_SURFACE_OPTIONS,
        CHIMERAX_VOLUME_OPERATIONS,
        build_feature_coverage,
    )
except ModuleNotFoundError:
    from scripts.graphics_engine_reference_manifest import (
        CHIMERAX_IMAGE_COMMANDS,
        CHIMERAX_SURFACE_OPTIONS,
        CHIMERAX_VOLUME_OPERATIONS,
        build_feature_coverage,
    )


DOC_LABELS = {
    "PyMOL": "PyMOL CGO",
    "VMD": "VMD",
    "ChimeraX": "ChimeraX",
    "OVITO": "OVITO",
    "Protein Imager": "Protein Imager",
}
STATUSES = ("missing", "partial")
TABLE_MARKER = "| Reference | Records marked `missing` | Records marked `partial` |"


def expected_records() -> dict[str, dict[str, list[str]]]:
    """Return ordered manifest records grouped by documentation row."""

    grouped: dict[str, dict[str, list[str]]] = defaultdict(
        lambda: {status: [] for status in STATUSES}
    )
    for record in build_feature_coverage({}):
        status = record["status"]
        if status in STATUSES:
            grouped[DOC_LABELS[record["source"]]][status].append(record["feature"])
    return dict(grouped)


def parse_table(document: str) -> dict[str, dict[str, list[str]]]:
    """Parse the compact missing/partial table from the parity document."""

    try:
        start = document.index(TABLE_MARKER)
    except ValueError:
        return {}
    rows: dict[str, dict[str, list[str]]] = {}
    for line in document[start:].splitlines()[2:]:
        if not line.startswith("|"):
            break
        cells = [cell.strip() for cell in line.strip().strip("|").split("|")]
        if len(cells) != 3 or cells[0].startswith("-"):
            continue
        rows[cells[0]] = {
            status: re.findall(r"`([^`]+)`", cells[index])
            for status, index in (("missing", 1), ("partial", 2))
        }
    return rows


def dict_keys_in_block(source: str, name: str, end_marker: str) -> list[str]:
    """Extract literal keys from a generated child-script dictionary."""

    start_marker = f"{name} = {{"
    try:
        start = source.index(start_marker) + len(start_marker)
        end = source.index(end_marker, start)
    except ValueError:
        return []
    return re.findall(r'^    "([^"]+)":', source[start:end], re.MULTILINE)


def runtime_recipe_audit(repo: Path) -> dict[str, Any]:
    """Check that ChimeraX's per-feature matrices have bounded recipes."""

    probe = (repo / "scripts/graphics_engine_chimerax_probe.py").read_text(
        encoding="utf-8"
    )
    families = (
        (
            "image-command",
            CHIMERAX_IMAGE_COMMANDS,
            dict_keys_in_block(probe, "IMAGE_COMMAND_SPECS", "\n}\n\n\nIMAGE_CONTEXTS"),
        ),
        (
            "volume-operation",
            CHIMERAX_VOLUME_OPERATIONS,
            dict_keys_in_block(probe, "volume_recipes", "\n}\n\nvolume_operation_records"),
        ),
        (
            "surface-option",
            CHIMERAX_SURFACE_OPTIONS,
            dict_keys_in_block(probe, "surface_recipes", "\n}\n\nsurface_option_records"),
        ),
    )
    results = {}
    issues: list[str] = []
    for family, expected, actual in families:
        missing = sorted(set(expected) - set(actual))
        extra = sorted(set(actual) - set(expected))
        if missing or extra:
            issues.append(f"{family} recipes missing={missing!r} extra={extra!r}")
        results[family] = {
            "manifest_count": len(expected),
            "recipe_count": len(actual),
            "missing": missing,
            "extra": extra,
        }
    return {"families": results, "issues": issues}


def audit(document_path: Path) -> dict[str, Any]:
    """Compare the manifest and documentation table without changing either."""

    document = document_path.read_text(encoding="utf-8")
    expected = expected_records()
    actual = parse_table(document)
    repo = document_path.resolve().parents[1]
    recipes = runtime_recipe_audit(repo)
    issues: list[str] = []
    for label, records in expected.items():
        if label not in actual:
            issues.append(f"missing documentation row: {label}")
            continue
        for status in STATUSES:
            if actual[label][status] != records[status]:
                issues.append(
                    f"{label}/{status}: expected {records[status]!r}, "
                    f"found {actual[label][status]!r}"
                )
    unexpected_rows = sorted(set(actual) - set(expected))
    if unexpected_rows:
        issues.append(f"unexpected documentation rows: {unexpected_rows!r}")
    issues.extend(recipes["issues"])
    return {
        "schema": 1,
        "status": "passed" if not issues else "failed",
        "document": str(document_path),
        "rows": {label: {status: len(values) for status, values in records.items()}
                 for label, records in expected.items()},
        "runtime_recipes": recipes["families"],
        "issues": issues,
    }


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--doc",
        type=Path,
        default=Path(__file__).resolve().parents[1] / "docs/GRAPHICS-ENGINE-PARITY.md",
    )
    parser.add_argument("--check", action="store_true")
    args = parser.parse_args()
    result = audit(args.doc.resolve())
    print(json.dumps(result, indent=2, sort_keys=True))
    return 1 if args.check and result["status"] != "passed" else 0


if __name__ == "__main__":
    raise SystemExit(main())
