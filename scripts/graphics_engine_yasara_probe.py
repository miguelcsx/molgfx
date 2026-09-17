#!/usr/bin/env python3
"""Emit a complete source-grounded YASARA command disposition report.

YASARA's full manual and native executable are not present in this checkout.
The vendor feature-list snapshot is therefore treated as source evidence, not
as successful renderer execution.  The report retains every unique parsed
command, the repeated raw source row, its molgfx owner/status decision, and an
explicit native ``not-run`` boundary.
"""

from __future__ import annotations

import argparse
import json
from collections import Counter
from pathlib import Path
from typing import Any

try:
    from graphics_engine_reference_inventory import parse_yasara, source_evidence
    from graphics_engine_reference_manifest import yasara_command_disposition
except ModuleNotFoundError:
    from scripts.graphics_engine_reference_inventory import parse_yasara, source_evidence
    from scripts.graphics_engine_reference_manifest import yasara_command_disposition


EXPECTED_SOURCE_ROWS = 481
EXPECTED_UNIQUE_COMMANDS = 480


def report(source: Path | None) -> dict[str, Any]:
    """Build the complete YASARA source/native boundary report."""

    parsed = parse_yasara(source)
    records: list[dict[str, Any]] = []
    for section, commands in parsed.get("commands_by_section", {}).items():
        for command in commands:
            status, owner = yasara_command_disposition(
                section, command["name"], command["description"]
            )
            records.append(
                {
                    "source": "YASARA",
                    "family": section,
                    "feature": command["name"],
                    "description": command["description"],
                    "status": status,
                    "owner": owner,
                    "evidence": "official feature-list source disposition; native execution not available",
                }
            )
    unreviewed = [record["feature"] for record in records if record["status"] == "unreviewed"]
    source_status = parsed.get("source", {}).get("status")
    return {
        "schema": 1,
        "status": "passed" if source_status == "read" and not unreviewed else "source-unavailable",
        "scope": "YASARA source inventory and renderer boundary; not native image parity",
        "source": parsed.get("source", source_evidence(source)),
        "source_row_count": parsed.get("source_command_row_count", 0),
        "unique_command_count": parsed.get("command_count", 0),
        "graphics_command_count": parsed.get("graphics_command_count", 0),
        "duplicate_source_rows": parsed.get("duplicate_commands", []),
        "native": {
            "status": "not-run",
            "reason": "YASARA executable and shipped full manual are unavailable on this host",
        },
        "status_counts": dict(sorted(Counter(record["status"] for record in records).items())),
        "unreviewed": unreviewed,
        "records": records,
    }


def check(result: dict[str, Any]) -> list[str]:
    """Validate the current source snapshot without claiming native parity."""

    issues: list[str] = []
    if result["source"].get("status") != "read":
        issues.append("YASARA feature-list source is not supplied")
    if result["source_row_count"] != EXPECTED_SOURCE_ROWS:
        issues.append(
            f"expected {EXPECTED_SOURCE_ROWS} source rows, found {result['source_row_count']}"
        )
    if result["unique_command_count"] != EXPECTED_UNIQUE_COMMANDS:
        issues.append(
            f"expected {EXPECTED_UNIQUE_COMMANDS} unique commands, found {result['unique_command_count']}"
        )
    if len(result["duplicate_source_rows"]) != 1:
        issues.append(
            f"expected one duplicate source row, found {len(result['duplicate_source_rows'])}"
        )
    if result["unreviewed"]:
        issues.append(f"unreviewed YASARA records: {result['unreviewed']!r}")
    if len(result["records"]) != result["unique_command_count"]:
        issues.append("unique command record count does not match parsed command count")
    return issues


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--features", type=Path, required=True)
    parser.add_argument("--output", type=Path)
    parser.add_argument("--check", action="store_true")
    parser.add_argument("--summary", action="store_true")
    args = parser.parse_args()
    result = report(args.features.resolve())
    issues = check(result)
    result["check_issues"] = issues
    if args.output:
        args.output.parent.mkdir(parents=True, exist_ok=True)
        args.output.write_text(json.dumps(result, indent=2, sort_keys=True) + "\n", encoding="utf-8")
    if args.summary:
        print(json.dumps({
            "status": result["status"],
            "source_row_count": result["source_row_count"],
            "unique_command_count": result["unique_command_count"],
            "graphics_command_count": result["graphics_command_count"],
            "duplicate_source_rows": len(result["duplicate_source_rows"]),
            "native_status": result["native"]["status"],
            "status_counts": result["status_counts"],
            "check_issues": issues,
        }, sort_keys=True))
    elif not args.output:
        print(json.dumps(result, indent=2, sort_keys=True))
    if args.check:
        return 1 if issues else 0
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
