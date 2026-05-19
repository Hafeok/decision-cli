#!/usr/bin/env bash
# scripts/checks/value-stream-scope.sh
#
# Enforces ADR-005 — value-stream scope is a graph-resident artifact loaded
# at command start, not a runtime flag. Mechanical check: the scope module
# exists and surfaces `ActiveScope::load(workdir)` plus the
# `UnauthorizedGoal` error variant that operators see at the §3.4 chokepoint.
#
# Exit 0: scope module is in place.
# Exit 1: ActiveScope::load or the UnauthorizedGoal variant has been
#         removed (the §3.4 chokepoint has regressed).
set -euo pipefail

if REPO_ROOT="$(git rev-parse --show-toplevel 2>/dev/null)"; then
  cd "$REPO_ROOT"
fi

SCOPE_DIR="crates/decision-cli/src/scope"
if [ ! -d "$SCOPE_DIR" ]; then
  echo "ERROR: expected $SCOPE_DIR (ADR-005 anchor)" >&2
  exit 1
fi

if ! grep -rq "pub fn load" "$SCOPE_DIR"; then
  echo "ERROR: scope module no longer exposes ActiveScope::load (ADR-005)"
  exit 1
fi

if ! grep -rq "UnauthorizedGoal" "$SCOPE_DIR"; then
  echo "ERROR: scope module no longer surfaces UnauthorizedGoal (ADR-005)"
  exit 1
fi

if ! grep -rq "validate_goal" "$SCOPE_DIR"; then
  echo "ERROR: scope module no longer surfaces validate_goal (ADR-005)"
  exit 1
fi

echo "OK: ActiveScope + UnauthorizedGoal chokepoint in place (ADR-005)"
exit 0
