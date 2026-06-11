#!/usr/bin/env bash
# TC-429 (FT-172): the hardened add-artifact-type audit rejects the
# witnessed FT-147 sandbox (committed at .dec/cluster/FT-147), whose
# vocab cell invented the decisionframework.com namespace. The
# pre-FT-172 audit passed this sandbox; the canonical_namespace check
# must fail it naming the offending file.
set -euo pipefail
cd "$(git rev-parse --show-toplevel)"
OUT=$(python3 scripts/checks/cluster-audit-add-artifact-type.py .dec/cluster/FT-147 \
  crates/dec-ontology/src/ontology/archetype.rs \
  crates/dec-ontology/src/vocab/archetype.rs 2>&1) && {
  echo "ERROR: audit passed the witnessed bad-namespace sandbox"; exit 1; }
echo "$OUT" | grep -q 'check=canonical_namespace' || {
  echo "ERROR: audit failed for the wrong reason: $OUT"; exit 1; }
echo "OK: canonical_namespace rejects the witnessed FT-147 drift"
