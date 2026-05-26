#!/usr/bin/env bash
# tests/scripts/tc-172-cross-cutting-adr-coverage.sh
#
# TC-172: Every cross-cutting ADR has at least one implementing feature linked,
# modulo documented exclusions.
#
# Two-tier exit-code contract (per ADR-013):
#   exit 0 — every accepted cross-cutting ADR has >=1 feature link (modulo the
#            documented exclusion + delegation set), AND the exclusion list
#            captured in FT-103's body is in sync with the script's hardcoded
#            set.
#   exit 1 — at least one assertion failed; diagnostic lines on stdout name
#            the offenders.
#
# Implementation note: the TC body authorises either a `product graph sparql`
# pass or a Python fallback that loads ADR frontmatter directly. product-cli
# exposes `product graph query` for SPARQL today, but the frontmatter walk is
# simpler, has no external dependency beyond Python 3, and exercises exactly
# the same assertion. We take the Python-fallback path.

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"
ADRS_DIR="$REPO_ROOT/.product/adrs"
FEATURES_DIR="$REPO_ROOT/.product/features"

if [ ! -d "$ADRS_DIR" ]; then
  echo "ERROR: .product/adrs/ not found at $ADRS_DIR"
  exit 1
fi

FT103_FILE="$(find "$FEATURES_DIR" -maxdepth 1 -name 'FT-103-*.md' -print -quit 2>/dev/null || true)"
if [ -z "$FT103_FILE" ] || [ ! -f "$FT103_FILE" ]; then
  echo "ERROR: FT-103 feature file not found under $FEATURES_DIR"
  exit 1
fi

python3 - "$ADRS_DIR" "$FT103_FILE" <<'PYEOF'
"""
TC-172 assertion runner.

Scenarios checked (matching the TC body):
  A) Every accepted ADR with scope: cross-cutting has >=1 feature in its
     `features:` frontmatter list, except for IDs on the exclusion +
     delegation set.
  B) The hardcoded exclusion set below appears verbatim in FT-103's body,
     so the rationale and the executable assertion cannot drift silently.
  C) Superseded ADRs are not counted (covered by the status filter).
  E) scope: cross-cutting is the canonical marker (re-scoped ADRs are
     out of test scope by construction).

Scenario D (predicate-name pinning) is implicit: this script reads
frontmatter keys directly, so the keys are pinned in source rather than
in a SPARQL string.
"""

import sys
import re
from pathlib import Path

adrs_dir = Path(sys.argv[1])
ft103_path = Path(sys.argv[2])

# Hardcoded exclusion + delegation set. Must match FT-103's body verbatim
# (scenario B keeps them in sync).
EXCLUDED = {
    # Excluded — forward-looking / cross-stream, no decision-cli implementer
    # expected:
    "ADR-065",  # Dagger deferred as worker runtime model
    "ADR-044",  # Brief as a typed artifact in product-cli's catalog
    # Delegated — will be satisfied by platform fitness functions, not by
    # per-slice implementer:
    "ADR-014",  # Architectural fitness functions tracked as product-cli artifacts
    "ADR-021",  # Action-interpretation agreement as fitness metric
}


def parse_frontmatter(text):
    """Parse a leading ---\n...\n--- YAML-ish block.

    Recognises scalar key: value pairs and `- item` list members under a key
    written as `key:` (possibly with trailing whitespace). Sufficient for the
    product-cli ADR/feature frontmatter shape; not a full YAML parser.
    """
    m = re.match(r'^---\n(.*?)\n---', text, re.DOTALL)
    if not m:
        return {}
    fm = {}
    current_list_key = None
    for raw in m.group(1).split('\n'):
        line = raw.rstrip()
        if not line:
            continue
        # list-item continuation under the previous key
        if line.startswith('- ') and current_list_key is not None:
            fm[current_list_key].append(line[2:].strip())
            continue
        kv = re.match(r'^([A-Za-z_][A-Za-z0-9_-]*):\s*(.*)$', line)
        if not kv:
            continue
        key, value = kv.group(1), kv.group(2).strip()
        if value == '' or value == '[]':
            fm[key] = []
            current_list_key = key
        else:
            fm[key] = value
            current_list_key = None
    return fm


# --- Scenario A: cross-cutting ADRs without an implementer ---------------

violations = []
for path in sorted(adrs_dir.glob('ADR-*.md')):
    text = path.read_text()
    fm = parse_frontmatter(text)
    adr_id = (fm.get('id') or '').strip()
    status = (fm.get('status') or '').strip()
    scope = (fm.get('scope') or '').strip()
    if not adr_id:
        continue
    if status != 'accepted':
        continue
    if scope != 'cross-cutting':
        continue
    if adr_id in EXCLUDED:
        continue
    features = fm.get('features') or []
    if not features:
        violations.append(adr_id)

# --- Scenario B: exclusion set must be reflected in FT-103's body --------

ft103_text = ft103_path.read_text()
missing_in_ft103 = sorted(adr for adr in EXCLUDED if adr not in ft103_text)

# --- Report --------------------------------------------------------------

exit_code = 0

if violations:
    exit_code = 1
    for v in sorted(violations):
        print(
            f"Cross-cutting ADR {v} has no implementing feature. "
            f"Either link the implementing feature "
            f"(product feature link FT-NNN --adr {v}) "
            f"or add {v} to FT-103's exclusion/delegation list with a justification."
        )

if missing_in_ft103:
    exit_code = 1
    print(
        "FT-103 exclusion list and TC-172 exclusion set out of sync; "
        "one was updated without the other. "
        f"Missing in FT-103: {missing_in_ft103}"
    )

if exit_code == 0:
    print(
        "OK: every accepted cross-cutting ADR has >=1 implementing feature "
        "(modulo {n} documented exclusions), and the exclusion set is "
        "reflected in FT-103.".format(n=len(EXCLUDED))
    )

sys.exit(exit_code)
PYEOF
