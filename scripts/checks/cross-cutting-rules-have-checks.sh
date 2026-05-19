#!/usr/bin/env bash
# scripts/checks/cross-cutting-rules-have-checks.sh
#
# Enforces ADR-014 — the convention that every cross-cutting ADR (every
# "rule" in decision-cli's terms) has at least one TC with a configured
# runner, so the rule is mechanically enforceable by `product verify
# --platform`.
#
# Walks every ADR in .product/adrs/ whose front-matter declares
# `scope: cross-cutting`. For each, scans .product/tests/ for at least one
# TC whose `validates.adrs` list contains the ADR id AND whose front-matter
# has a non-empty `runner:` field.
#
# Exit 0: every cross-cutting ADR has at least one TC with a runner.
# Exit 2: at least one cross-cutting ADR has no runner-equipped TC. This is
#         a warning, not a hard failure: the cross-cutting ADRs predating
#         ADR-014 may legitimately have no mechanical check (e.g. SDP
#         boundary enforced by code review). New rule ADRs added under
#         ADR-014's convention must pair with a runner TC; the workflow
#         gate in product-cli (FT-058) is the primary enforcement. This
#         script is a backstop and reports drift, not a self-veto.
#
# Diagnostics go to stdout; script-self errors go to stderr.
set -euo pipefail

if REPO_ROOT="$(git rev-parse --show-toplevel 2>/dev/null)"; then
  cd "$REPO_ROOT"
fi

ADR_DIR=".product/adrs"
TC_DIR=".product/tests"

if [ ! -d "$ADR_DIR" ] || [ ! -d "$TC_DIR" ]; then
  echo "ERROR: expected $ADR_DIR and $TC_DIR to exist (run from repo root)" >&2
  exit 1
fi

# Collect cross-cutting ADR ids by reading their front-matter.
CROSS_CUTTING_IDS=""
for f in "$ADR_DIR"/ADR-*.md; do
  [ -f "$f" ] || continue
  # Pull front-matter (between the first two `---` lines).
  fm="$(awk 'BEGIN{c=0} /^---$/{c++; next} c==1{print}' "$f")"
  scope="$(echo "$fm" | awk '/^scope:/{print $2; exit}')"
  if [ "$scope" = "cross-cutting" ]; then
    id="$(echo "$fm" | awk '/^id:/{print $2; exit}')"
    CROSS_CUTTING_IDS="$CROSS_CUTTING_IDS $id"
  fi
done

if [ -z "${CROSS_CUTTING_IDS// }" ]; then
  echo "OK: no cross-cutting ADRs found (vacuously satisfied)"
  exit 0
fi

# For each TC, parse front-matter once and record (adr_id -> runner_present).
# A TC counts toward the invariant iff:
#   - its front-matter's validates.adrs list contains the ADR id
#   - its front-matter has a `runner:` line with a non-empty value
MISSING=""
for adr_id in $CROSS_CUTTING_IDS; do
  found=0
  for tc in "$TC_DIR"/TC-*.md; do
    [ -f "$tc" ] || continue
    fm="$(awk 'BEGIN{c=0} /^---$/{c++; next} c==1{print}' "$tc")"
    runner="$(echo "$fm" | awk -F: '/^runner:/{sub(/^[[:space:]]+/,"",$2); print $2; exit}')"
    [ -z "$runner" ] && continue
    # Look for `- ADR-id` inside the `validates: adrs:` list. We grep the
    # list by anchoring on the indented dash form product-cli writes.
    if echo "$fm" | grep -Eq "^[[:space:]]*-[[:space:]]+${adr_id}\b"; then
      found=1
      break
    fi
  done
  if [ "$found" -eq 0 ]; then
    MISSING="$MISSING $adr_id"
  fi
done

if [ -n "${MISSING// }" ]; then
  echo "WARNING: cross-cutting ADRs without a TC carrying a runner:"
  for id in $MISSING; do
    echo "  $id — no TC found whose validates.adrs contains $id and whose runner is set"
  done
  echo
  echo "Per ADR-014, every cross-cutting ADR that codifies a mechanically"
  echo "checkable rule should be backed by at least one TC with"
  echo "runner: <bash|pytest|cargo-test|custom> so that"
  echo "\`product verify --platform\` enforces the rule. ADRs that codify"
  echo "non-mechanical constraints (e.g. SDP boundary enforced by review)"
  echo "may legitimately remain runnerless and produce only this warning."
  exit 2
fi

count="$(echo "$CROSS_CUTTING_IDS" | wc -w | tr -d ' ')"
echo "OK: all $count cross-cutting ADR(s) have at least one TC with a runner"
exit 0
