#!/usr/bin/env bash
# Validates the specification set against docs/MANIFEST.txt:
#   1. every listed path exists;
#   2. every numbered document carries the Status/Normative front-matter block;
#   3. every Discharges/Records reference resolves (FR/NFR ids into
#      01-requirements.md, ADR ids into docs/adr/).
set -uo pipefail
cd "$(dirname "$0")/.."

fail=0
manifest="docs/MANIFEST.txt"
[ -f "$manifest" ] || { echo "missing $manifest"; exit 1; }

# 1. Listed paths exist.
while IFS= read -r line; do
  path=$(echo "$line" | sed -n 's/^[[:space:]]*\(\(adr\/\)\?[0-9A-Za-z][0-9A-Za-z._-]*\.\(md\|txt\)\).*$/\1/p')
  [ -n "$path" ] || continue
  if [ ! -f "docs/$path" ]; then
    echo "manifest lists docs/$path but it does not exist"; fail=1
  fi
done < "$manifest"

# 2. Numbered docs carry front-matter.
for doc in docs/[0-9][0-9]-*.md docs/adr/[0-9][0-9][0-9][0-9]-*.md; do
  [ -f "$doc" ] || continue
  case "$doc" in docs/adr/0000-template.md) continue;; esac
  if ! grep -q "^\*\*Status:\*\*" "$doc"; then
    echo "$doc: missing Status front-matter"; fail=1
  fi
  case "$doc" in
    docs/adr/*) ;;  # ADRs carry Status only
    *) if ! grep -q "^\*\*Normative:\*\*" "$doc"; then
         echo "$doc: missing Normative front-matter"; fail=1
       fi;;
  esac
done

# 3. Discharges/Records references resolve.
refs=$(grep -rhoE "^\*\*(Discharges|Records):\*\*.*" docs --include="*.md" \
  | grep -oE "(FR|NFR|GS)-[0-9]+|ADR-[0-9]{4}" | sort -u)
for ref in $refs; do
  case "$ref" in
    ADR-*)
      n="${ref#ADR-}"
      ls docs/adr/"$n"-*.md >/dev/null 2>&1 || { echo "dangling reference $ref"; fail=1; };;
    *)
      grep -q "$ref" docs/01-requirements.md docs/23-benchmarks.md 2>/dev/null \
        || { echo "dangling reference $ref"; fail=1; };;
  esac
done

if [ "$fail" -ne 0 ]; then echo "check-docs: FAIL"; exit 1; fi
echo "check-docs: OK"
