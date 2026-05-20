#!/usr/bin/env bash
# scripts/checks/vertical-slice-imports.sh
#
# Enforces ADR-016 — Vertical-slice architecture with compile-time SDP
# enforcement. Sibling features under `crates/decision-cli/src/features/`
# MUST NOT import from each other; shared code is promoted into `core/`
# per the ADR's promotion rule.
#
# Walks `crates/decision-cli/src/features/*/**/*.rs`, infers the owning
# feature from each path, and flags any `use crate::features::<other>::`
# or `use super::super::<other>::` pattern that crosses feature
# boundaries. Wildcard reaches (`use crate::features::*`) from inside a
# feature directory are also flagged.
#
# Exit 0: features tree is clean.
# Exit 1: at least one forbidden cross-feature import found. Diagnostic
#         lines on stdout name the offending file/line/import.
set -euo pipefail

if REPO_ROOT="$(git rev-parse --show-toplevel 2>/dev/null)"; then
  cd "$REPO_ROOT"
fi

FEATURES_DIR="crates/decision-cli/src/features"

if [ ! -d "$FEATURES_DIR" ]; then
  echo "OK: $FEATURES_DIR does not exist; nothing to check"
  exit 0
fi

VIOLATIONS=""

while IFS= read -r file; do
  [ -z "$file" ] && continue
  rel="${file#"$FEATURES_DIR"/}"
  owning_feature="${rel%%/*}"
  [ "$owning_feature" = "mod.rs" ] && continue

  # Pattern 1: `use crate::features::<sibling>::` where <sibling> != owning_feature.
  while IFS= read -r match; do
    [ -z "$match" ] && continue
    line_no="${match%%:*}"
    body="${match#*:}"
    # Extract the target feature segment after `crate::features::`.
    target="$(printf '%s' "$body" | sed -nE 's|.*use[[:space:]]+crate::features::([A-Za-z_][A-Za-z0-9_]*).*|\1|p')"
    if [ -n "$target" ] && [ "$target" != "$owning_feature" ]; then
      VIOLATIONS="${VIOLATIONS}${file}:${line_no}: cross-feature import (use crate::features::${target}) inside ${owning_feature}: ${body}"$'\n'
    fi
  done < <(grep -nE '^[[:space:]]*use[[:space:]]+crate::features::' "$file" || true)

  # Pattern 2: `use crate::features::*` (wildcard).
  while IFS= read -r match; do
    [ -z "$match" ] && continue
    line_no="${match%%:*}"
    body="${match#*:}"
    VIOLATIONS="${VIOLATIONS}${file}:${line_no}: wildcard reach into crate::features::* inside ${owning_feature}: ${body}"$'\n'
  done < <(grep -nE '^[[:space:]]*use[[:space:]]+crate::features::\*' "$file" || true)

  # Pattern 3: `use super::super::<sibling>::` — only meaningful at depth
  # >= 2 (features/<feat>/<file>.rs lands at super::super == features).
  while IFS= read -r match; do
    [ -z "$match" ] && continue
    line_no="${match%%:*}"
    body="${match#*:}"
    target="$(printf '%s' "$body" | sed -nE 's|.*use[[:space:]]+super::super::([A-Za-z_][A-Za-z0-9_]*).*|\1|p')"
    if [ -n "$target" ] && [ "$target" != "$owning_feature" ]; then
      VIOLATIONS="${VIOLATIONS}${file}:${line_no}: cross-feature import (use super::super::${target}) inside ${owning_feature}: ${body}"$'\n'
    fi
  done < <(grep -nE '^[[:space:]]*use[[:space:]]+super::super::' "$file" || true)
done < <(find "$FEATURES_DIR" -mindepth 2 -name '*.rs' | sort)

if [ -n "$VIOLATIONS" ]; then
  echo "ERROR: cross-feature imports forbidden by ADR-016:"
  printf '%s' "$VIOLATIONS" | while IFS= read -r line; do
    [ -z "$line" ] && continue
    echo "  $line"
  done
  echo ""
  echo "Per ADR-016, sibling features must depend only on core/, not on each"
  echo "other. Promote the shared code into core/ first; both features then"
  echo "import from core/."
  exit 1
fi

echo "OK: features tree contains no cross-feature imports"
exit 0
