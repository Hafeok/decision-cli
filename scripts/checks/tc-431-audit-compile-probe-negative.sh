#!/usr/bin/env bash
# TC-431 (FT-172): the compile probe rejects emitted Rust that does not
# type-check against HEAD, carrying the rustc diagnostic in the FAIL line.
set -euo pipefail
cd "$(git rev-parse --show-toplevel)"
FIX=$(mktemp -d); trap 'rm -rf "$FIX"' EXIT
CELLS=(
  crates/dec-ontology/src/ontology/archetype/types.rs
  crates/dec-ontology/src/ontology/shapes/archetype.shacl.ttl
  crates/dec-ontology/src/vocab/archetype.rs
)
for p in "${CELLS[@]}"; do mkdir -p "$FIX/$(dirname "$p")"; cp "$p" "$FIX/$p"; done
echo 'pub fn broken( -> {' >> "$FIX/crates/dec-ontology/src/vocab/archetype.rs"
OUT=$(python3 scripts/checks/cluster-audit-add-artifact-type.py "$FIX" "${CELLS[@]}" 2>&1) && {
  echo "ERROR: audit passed non-compiling Rust"; exit 1; }
echo "$OUT" | grep -q 'check=compile_probe' || {
  echo "ERROR: audit failed for the wrong reason: $OUT"; exit 1; }
echo "OK: compile probe rejects non-compiling emissions"
