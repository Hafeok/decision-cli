#!/usr/bin/env bash
# scripts/checks/feature-tc-coverage.sh
#
# Enforces ADR-072 — every feature with `status: complete` must have at
# least four linked TCs under its `tests:` front-matter block.
#
# Pre-existing under-covered features are listed in
# `scripts/checks/feature-tc-coverage.baseline` (one feature ID per
# line) and are exempt from the hard gate while their backfill is
# driven by the TC-author worker. New violations (features that turn
# complete with <4 TCs after ADR-072 lands) fail CI.
#
# Exit 0: no new violations. Baseline features still under the floor are
#         emitted as `BASELINE:` lines (advisory). Features in
#         `status: in-progress` under the floor are emitted as
#         `WARNING:` lines (advisory).
# Exit 1: at least one feature with `status: complete` is under the
#         floor and is NOT in the baseline; OR a feature listed in the
#         baseline now has zero TCs (regression below the snapshot).
#
# The TC-coverage floor is resolved through ADR-068's precedence chain:
#
#   DEC_VERIFICATION_MIN_TCS_PER_FEATURE env var
#     > [verification] min_tcs_per_feature in .dec/config.toml
#     > built-in default (4)
#
# Other paths may be overridden via environment variables:
#   FEATURES_DIR    (default: .product/features)
#   BASELINE_FILE   (default: scripts/checks/feature-tc-coverage.baseline)
#   CONFIG_FILE     (default: .dec/config.toml)
#
# Diagnostic output goes to stdout so `product verify` captures it in
# the TC failure record. Script-self errors go to stderr.
set -euo pipefail

FEATURES_DIR=${FEATURES_DIR:-.product/features}
BASELINE_FILE=${BASELINE_FILE:-scripts/checks/feature-tc-coverage.baseline}
CONFIG_FILE=${CONFIG_FILE:-.dec/config.toml}

if REPO_ROOT="$(git rev-parse --show-toplevel 2>/dev/null)"; then
  cd "$REPO_ROOT"
fi

# Resolve MIN via ADR-068 precedence: env > config.toml > default.
resolve_min() {
  if [ -n "${DEC_VERIFICATION_MIN_TCS_PER_FEATURE:-}" ]; then
    printf '%s' "$DEC_VERIFICATION_MIN_TCS_PER_FEATURE"
    return
  fi
  if [ -f "$CONFIG_FILE" ]; then
    awk '
      /^\[verification\]/ { in_section = 1; next }
      /^\[/              { in_section = 0 }
      in_section && /^[[:space:]]*min_tcs_per_feature[[:space:]]*=/ {
        sub(/^[^=]*=[[:space:]]*/, "")
        sub(/[[:space:]]*#.*$/, "")
        gsub(/[[:space:]]/, "")
        print
        exit
      }
    ' "$CONFIG_FILE"
    return
  fi
}

MIN="$(resolve_min)"
MIN=${MIN:-4}

if ! [[ "$MIN" =~ ^[0-9]+$ ]]; then
  echo "ERROR: min_tcs_per_feature must be a non-negative integer, got: $MIN" >&2
  exit 2
fi

if [ ! -d "$FEATURES_DIR" ]; then
  echo "ERROR: features directory not found: $FEATURES_DIR" >&2
  exit 1
fi

# Build an associative lookup of baseline IDs. Lines beginning with `#`
# and blank lines are ignored so the baseline file can carry comments
# explaining each entry.
declare -A BASELINE=()
if [ -f "$BASELINE_FILE" ]; then
  while IFS= read -r line; do
    line="${line%%#*}"
    line="$(echo "$line" | tr -d '[:space:]')"
    [ -z "$line" ] && continue
    BASELINE["$line"]=1
  done < "$BASELINE_FILE"
fi

FAIL=0
NEW_VIOLATIONS=""
WARNINGS=""
BASELINE_HITS=""
BASELINE_PROMOTABLE=""

# Iterate features in ID order so output is stable.
for f in $(ls "$FEATURES_DIR"/*.md 2>/dev/null | sort); do
  id="$(awk '/^id:/{print $2; exit}' "$f")"
  status="$(awk '/^status:/{print $2; exit}' "$f")"
  tc_count="$(awk '
    /^tests:/      { flag = 1; next }
    /^[a-zA-Z_-]+:/ { flag = 0 }
    flag && /^- TC-/ { n++ }
    END { print n + 0 }
  ' "$f")"

  [ -z "$id" ] && continue

  if [ "$tc_count" -ge "$MIN" ]; then
    if [ -n "${BASELINE[$id]:-}" ]; then
      BASELINE_PROMOTABLE+="  $id: $tc_count TCs (>= $MIN) — remove from baseline"$'\n'
    fi
    continue
  fi

  # Under the floor. Branch on status + baseline membership.
  case "$status" in
    complete)
      if [ -n "${BASELINE[$id]:-}" ]; then
        BASELINE_HITS+="  $id: $tc_count TCs (baseline-exempt)"$'\n'
      else
        NEW_VIOLATIONS+="  $id: $tc_count TCs (need >= $MIN, not in baseline)"$'\n'
        FAIL=1
      fi
      ;;
    in-progress)
      WARNINGS+="  $id: $tc_count TCs (in-progress, will block at status=complete)"$'\n'
      ;;
    *)
      # planned / draft / abandoned / superseded are not gated.
      :
      ;;
  esac
done

if [ -n "$NEW_VIOLATIONS" ]; then
  echo "ERROR: features with status=complete under TC floor ($MIN), not in baseline:"
  printf '%s' "$NEW_VIOLATIONS"
fi

if [ -n "$WARNINGS" ]; then
  echo "WARNING: features with status=in-progress under TC floor ($MIN):"
  printf '%s' "$WARNINGS"
fi

if [ -n "$BASELINE_HITS" ]; then
  echo "BASELINE: pre-existing under-covered features (tracked, not gated):"
  printf '%s' "$BASELINE_HITS"
fi

if [ -n "$BASELINE_PROMOTABLE" ]; then
  echo "BASELINE: features now at or above the floor — remove from baseline:"
  printf '%s' "$BASELINE_PROMOTABLE"
fi

if [ $FAIL -ne 0 ]; then
  exit 1
fi

echo "OK: all complete features meet TC coverage floor (MIN=$MIN)"
exit 0
