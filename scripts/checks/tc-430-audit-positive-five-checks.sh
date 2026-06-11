#!/usr/bin/env bash
# TC-430 (FT-172): a correct cell set — the promoted FT-147 archetype
# files from the live tree — passes all five audit checks including the
# worktree compile probe.
set -euo pipefail
cd "$(git rev-parse --show-toplevel)"
FIX=$(mktemp -d); trap 'rm -rf "$FIX"' EXIT
CELLS=(
  crates/dec-ontology/src/ontology/archetype/types.rs
  crates/dec-ontology/src/ontology/shapes/archetype.shacl.ttl
  crates/dec-ontology/src/vocab/archetype.rs
  crates/dec-ontology/src/ontology/archetype/parser.rs
  crates/dec-ontology/src/ontology/archetype/emitter.rs
  crates/dec-ontology/src/ontology/archetype/tests.rs
)
for p in "${CELLS[@]}"; do mkdir -p "$FIX/$(dirname "$p")"; cp "$p" "$FIX/$p"; done
python3 scripts/checks/cluster-audit-add-artifact-type.py "$FIX" "${CELLS[@]}" \
  | grep -q 'PASS add-artifact-type (5 checks passed)'
echo "OK: promoted cell set passes all five checks"
