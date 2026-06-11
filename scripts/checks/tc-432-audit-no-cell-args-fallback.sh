#!/usr/bin/env bash
# TC-432 (FT-172): without the harness-passed cell list the audit
# degrades to auditing every .rs/.ttl in the fixture — pre-FT-172
# invocations keep working.
set -euo pipefail
cd "$(git rev-parse --show-toplevel)"
FIX=$(mktemp -d); trap 'rm -rf "$FIX"' EXIT
for p in \
  crates/dec-ontology/src/ontology/archetype/types.rs \
  crates/dec-ontology/src/ontology/shapes/archetype.shacl.ttl \
  crates/dec-ontology/src/vocab/archetype.rs; do
  mkdir -p "$FIX/$(dirname "$p")"; cp "$p" "$FIX/$p"
done
python3 scripts/checks/cluster-audit-add-artifact-type.py "$FIX" \
  | grep -q 'PASS add-artifact-type (5 checks passed)'
echo "OK: no-cell-args fallback audits the whole fixture"
