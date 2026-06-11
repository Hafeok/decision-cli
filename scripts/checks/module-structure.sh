#!/usr/bin/env bash
# scripts/checks/module-structure.sh
#
# Enforces ADR-013 Rule 3 — Module Decomposition. Two structural assertions:
#
#   (a) Each Rust crate under `crates/*/` declares the canonical core
#       top-level modules (see ADR-013 §"Module Decomposition"). The check
#       is conservative: it asserts the *presence* of well-known modules,
#       not the absence of additional ones, so the layout can grow
#       additively (e.g. by the FT-018 vertical-slice migration) without
#       this check failing in the interim.
#
#   (b) `crates/decision-cli/src/main.rs` is at most 80 lines (ADR-013's
#       hard cap on the binary entry point).
#
# Exit 0: structure is intact.
# Exit 1: structural violation found (missing canonical module or
#         main.rs too long).
# Exit 2: reserved for future warning conditions; not used today.
#
# Diagnostic output goes to stdout. Script-self errors go to stderr.
set -euo pipefail

MAIN_LIMIT=${MAIN_RS_LINE_LIMIT:-80}

if REPO_ROOT="$(git rev-parse --show-toplevel 2>/dev/null)"; then
  cd "$REPO_ROOT"
fi

FAILED=0

# -- (a) Canonical modules per crate ---------------------------------------
#
# A "module" is satisfied if either `crates/<crate>/src/<name>.rs` exists
# or `crates/<crate>/src/<name>/mod.rs` exists. We accept either form so
# crates can split a module into a directory without tripping the check.
module_present() {
  local crate="$1" name="$2"
  # Accept the canonical flat layout first.
  [ -f "crates/$crate/src/$name.rs" ] && return 0
  [ -f "crates/$crate/src/$name/mod.rs" ] && return 0
  # Post-FT-018 / ADR-016 layout: canonical modules may live under
  # `core/` (stable substrate) or `features/<name>/` (vertical slices).
  [ -f "crates/$crate/src/core/$name.rs" ] && return 0
  [ -f "crates/$crate/src/core/$name/mod.rs" ] && return 0
  [ -f "crates/$crate/src/features/$name.rs" ] && return 0
  [ -f "crates/$crate/src/features/$name/mod.rs" ] && return 0
  return 1
}

check_crate_modules() {
  local crate="$1"
  shift
  local missing=""
  for name in "$@"; do
    if ! module_present "$crate" "$name"; then
      missing="$missing $name"
    fi
  done
  if [ -n "${missing// }" ]; then
    echo "ERROR: crates/$crate/src/ missing canonical module(s):"
    for name in $missing; do
      echo "  - $name (expected crates/$crate/src/$name.rs or crates/$crate/src/$name/mod.rs)"
    done
    FAILED=1
  fi
}

# oxi-events canonical modules per ADR-013 §"Module Decomposition".
# The substrate's load-bearing surface: writer (mutation chokepoint),
# subscription (registry+evaluator), replay (SPARQL-based replay), and
# outbox (publisher background task).
if [ -d crates/oxi-events ]; then
  check_crate_modules oxi-events writer subscription replay outbox
fi

# decision-cli's per-ADR-013 canonical modules are aspirational and
# overlap with the FT-018 vertical-slice migration. We assert the module
# that is stable across layouts: the init slice. Ontology and vocab
# moved down the ADR-086 stack (FT-167/FT-168) and are asserted in
# their owning crates below.
if [ -d crates/decision-cli ]; then
  check_crate_modules decision-cli init
fi

# dec-ontology canonical modules per ADR-086: the pure domain layer owns
# the typed artifact submodules and the IRI vocabulary.
if [ -d crates/dec-ontology ]; then
  check_crate_modules dec-ontology ontology vocab
fi

# dec-graph canonical modules per ADR-086: store access, the stream
# writer chokepoint, and the store-facing ontology glue.
if [ -d crates/dec-graph ]; then
  check_crate_modules dec-graph store stream_writer graph ontology
fi

# -- (b) main.rs line cap --------------------------------------------------
MAIN_RS="crates/decision-cli/src/main.rs"
if [ -f "$MAIN_RS" ]; then
  count="$(wc -l < "$MAIN_RS" | tr -d ' ')"
  if [ "$count" -gt "$MAIN_LIMIT" ]; then
    echo "ERROR: $MAIN_RS has $count lines (limit: $MAIN_LIMIT)"
    echo "  ADR-013 mandates main.rs is dispatch only — no business logic."
    FAILED=1
  fi
fi

if [ "$FAILED" -ne 0 ]; then
  exit 1
fi

echo "OK: module structure conforms to ADR-013 (main.rs $count / $MAIN_LIMIT lines)"
exit 0
