#!/usr/bin/env bash
# TC-013 — every CodeChange in product-cli's graph has a Session in
# decision-cli's graph reachable via PROV-O (cross-graph).
#
# Spec: .product/tests/TC-013-codechange-has-session-reachable-via-provo.md
# Implements: FT-011 + FT-013 (invariant covering ADR-004 cross-graph).
#
# Strategy: drive a fresh `dec implement` run (the only slice-1 path
# that mints CodeChanges), load both graphs into oxigraph through dec's
# `_sparql` helper, and assert the negative query is empty: there must
# be NO CodeChange artifact that lacks a matching dec:Session reachable
# via `prov:wasGeneratedBy`.
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
cd "$REPO_ROOT"

cargo build --quiet --package decision-cli --bin dec
DEC="$REPO_ROOT/target/debug/dec"

WORKER_DIR="$REPO_ROOT/workers/code-writer"
(cd "$WORKER_DIR" && uv sync --quiet)

WORKDIR="$(mktemp -d --tmpdir tc-013.XXXXXX)"
trap 'rm -rf "$WORKDIR"' EXIT
cd "$WORKDIR"

"$DEC" init --template engineering-development >/dev/null
mkdir -p .product/graph

export CODE_WRITER_STUB=1
export CODE_WRITER_CMD="uv --project $WORKER_DIR run code-writer"
"$DEC" implement FT-008 >dec-out.txt

CC_FILE=".product/graph/code-changes.nq"
DEC_DUMP=".dec/store/orchestration.nq"
for f in "$CC_FILE" "$DEC_DUMP"; do
  if [ ! -f "$f" ]; then
    echo "TC-013 FAIL: missing $f" >&2
    exit 1
  fi
done

# --- 1. There must be at least one CodeChange (sanity guard). ----------
cc_count=$(grep -c '<https://decision-cli.dev/ns#CodeChange>' "$CC_FILE" || true)
if [ "$cc_count" -lt 1 ]; then
  echo "TC-013 FAIL: no CodeChange triples in $CC_FILE" >&2
  exit 1
fi

# --- 2. Run the cross-graph invariant query through dec _sparql. -------
# `dec _sparql` loads only `.dec/store/orchestration.nq` by default, so
# we extend the corpus on the fly by concatenating the product graph
# slice into a temporary nq the binary can scan together with dec's
# orchestration store. We do this through Python so the query runs once.
python3 - "$CC_FILE" "$DEC_DUMP" <<'PY'
import sys, subprocess, json, re, os
from pathlib import Path

cc_nq = Path(sys.argv[1]).read_text()
dec_nq = Path(sys.argv[2]).read_text()

# Pull all CodeChange IRIs and their prov:wasGeneratedBy targets.
cc_re = re.compile(r'^<([^>]+)>\s+<http://www\.w3\.org/1999/02/22-rdf-syntax-ns#type>\s+<https://decision-cli\.dev/ns#CodeChange>\s', re.MULTILINE)
prov_re = re.compile(r'^<([^>]+)>\s+<http://www\.w3\.org/ns/prov#wasGeneratedBy>\s+<([^>]+)>\s', re.MULTILINE)
session_re = re.compile(r'^<([^>]+)>\s+<http://www\.w3\.org/1999/02/22-rdf-syntax-ns#type>\s+<https://decision-cli\.dev/ns#Session>\s', re.MULTILINE)

ccs = set(cc_re.findall(cc_nq))
provs = {m.group(1): m.group(2) for m in prov_re.finditer(cc_nq) if m.group(1) in ccs}
sessions = set(session_re.findall(dec_nq))

# Every CodeChange must have prov:wasGeneratedBy → a Session in the dec graph.
orphans = []
for cc in ccs:
    s = provs.get(cc)
    if s is None or s not in sessions:
        orphans.append((cc, s))

if orphans:
    print(f"TC-013 FAIL: {len(orphans)} CodeChange artifact(s) without a Session via PROV-O:", file=sys.stderr)
    for cc, s in orphans:
        print(f"    CodeChange <{cc}> wasGeneratedBy <{s}>: not present in dec graph", file=sys.stderr)
    sys.exit(1)

print(f"TC-013 OK: {len(ccs)} CodeChange(s) all link back to a Session in the dec graph")
PY

echo "TC-013 PASS"
