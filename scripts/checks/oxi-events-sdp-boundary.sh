#!/usr/bin/env bash
# scripts/checks/oxi-events-sdp-boundary.sh
#
# Enforces ADR-001 Stable Dependency Principle: the `oxi-events` crate must
# not depend on `decision-cli` and must not reference DDD vocabulary
# (roles, bundles, sessions, policies, autonomy levels) in its public or
# private code.
#
# Exit 0: SDP boundary is intact.
# Exit 1: a dependency on decision-cli or a forbidden DDD term was found
#         in oxi-events sources or its manifest.
#
# Diagnostic output goes to stdout so `product verify` captures it.
set -euo pipefail

if REPO_ROOT="$(git rev-parse --show-toplevel 2>/dev/null)"; then
  cd "$REPO_ROOT"
fi

CARGO_TOML="crates/oxi-events/Cargo.toml"
if [ ! -f "$CARGO_TOML" ]; then
  echo "ERROR: expected $CARGO_TOML (run from repo root)" >&2
  exit 1
fi

FAILED=0

if grep -qE '^\s*decision[-_]cli\s*=' "$CARGO_TOML"; then
  echo "ERROR: $CARGO_TOML depends on decision-cli (ADR-001 violation)"
  FAILED=1
fi

# Public surface and source files: scan for DDD vocabulary that is
# expressly excluded by ADR-001. Comments are allowed (the file headers
# explain *why* the term is forbidden) so we look only at code lines.
FORBIDDEN_TERMS="role_id|RoleBinding|bundle_hash|session_id|policy_id|autonomy_level"
HITS="$(grep -rEn "$FORBIDDEN_TERMS" crates/oxi-events/src 2>/dev/null \
  | grep -vE '^[^:]+:[0-9]+:\s*//' || true)"
if [ -n "$HITS" ]; then
  echo "ERROR: forbidden DDD vocabulary in oxi-events sources (ADR-001):"
  echo "$HITS" | sed 's/^/  /'
  FAILED=1
fi

if [ "$FAILED" -ne 0 ]; then
  exit 1
fi

echo "OK: oxi-events SDP boundary is intact"
exit 0
