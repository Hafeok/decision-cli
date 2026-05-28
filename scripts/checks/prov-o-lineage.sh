#!/usr/bin/env bash
# scripts/checks/prov-o-lineage.sh
#
# Enforces ADR-004 — every Event and Session committed to the orchestration
# store carries PROV-O lineage. Mechanical check: the writer and init
# pipelines reference the canonical PROV-O predicates (`prov:wasGeneratedBy`,
# `prov:wasDerivedFrom`, `prov:atTime`).
#
# Exit 0: PROV-O predicates appear in the writer + init persistence code.
# Exit 1: a canonical PROV-O predicate has been removed (regression).
set -euo pipefail

if REPO_ROOT="$(git rev-parse --show-toplevel 2>/dev/null)"; then
  cd "$REPO_ROOT"
fi

WRITER_DIR="crates/oxi-events/src/writer"
INIT_DIR="crates/decision-cli/src/features/init"

for dir in "$WRITER_DIR" "$INIT_DIR"; do
  if [ ! -d "$dir" ]; then
    echo "ERROR: expected $dir (ADR-004 anchor)" >&2
    exit 1
  fi
done

FAILED=0
for pred in "prov#wasGeneratedBy" "prov#wasDerivedFrom" "prov#atTime"; do
  if ! grep -rq "$pred" "$WRITER_DIR" "$INIT_DIR"; then
    echo "ERROR: PROV-O predicate <$pred> missing from writer/init (ADR-004)"
    FAILED=1
  fi
done

if [ "$FAILED" -ne 0 ]; then
  exit 1
fi

echo "OK: writer + init reference PROV-O predicates (ADR-004)"
exit 0
