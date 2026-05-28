#!/usr/bin/env bash
# Module structure enforcement for ADR-013 Rule 3.
# Exits 0 if every required canonical module is present in each crate
# AND crates/decision-cli/src/main.rs is at most MAIN_RS_LINE_LIMIT lines (default 80).
# Exits 1 if a canonical module is missing or main.rs exceeds the cap.

set -euo pipefail

# Configurable main.rs line limit
MAIN_RS_LINE_LIMIT="${MAIN_RS_LINE_LIMIT:-80}"

# Track exit code
exit_code=0

# Canonical modules for oxi-events
OXI_EVENTS_MODULES=("writer" "subscription" "replay" "outbox")

# Canonical modules for decision-cli (intersection that survives FT-018)
DECISION_CLI_MODULES=("ontology" "init" "vocab")

# Check if a module exists (either as <name>.rs or <name>/mod.rs)
check_module() {
  local crate_path="$1"
  local module_name="$2"

  if [[ -f "$crate_path/$module_name.rs" ]] || [[ -f "$crate_path/$module_name/mod.rs" ]]; then
    return 0
  else
    return 1
  fi
}

# Check oxi-events modules
if [[ -d "crates/oxi-events/src" ]]; then
  for module in "${OXI_EVENTS_MODULES[@]}"; do
    if ! check_module "crates/oxi-events/src" "$module"; then
      echo "ERROR: Required module '$module' missing in crates/oxi-events/src/"
      exit_code=1
    fi
  done
fi

# Check decision-cli modules
if [[ -d "crates/decision-cli/src" ]]; then
  for module in "${DECISION_CLI_MODULES[@]}"; do
    if ! check_module "crates/decision-cli/src" "$module"; then
      echo "ERROR: Required module '$module' missing in crates/decision-cli/src/"
      exit_code=1
    fi
  done
fi

# Check main.rs line count
MAIN_RS="crates/decision-cli/src/main.rs"
if [[ -f "$MAIN_RS" ]]; then
  main_lines=$(wc -l < "$MAIN_RS")
  if (( main_lines > MAIN_RS_LINE_LIMIT )); then
    echo "ERROR: $MAIN_RS has $main_lines lines (limit $MAIN_RS_LINE_LIMIT)"
    exit_code=1
  fi
fi

exit $exit_code
