#!/usr/bin/env bash
# TC-008 — dec implement FT-XXX produces CodeChange + Session linked by PROV-O.
#
# Spec: .product/tests/TC-008-dec-implement-produces-codechange-session-and-prov.md
# Implements: FT-011 (implementer harness) + FT-013 (Python code-writer worker).
#
# This TC drives the worker in stub mode (CODE_WRITER_STUB=1) so the
# end-to-end harness flow is exercised without depending on a Claude
# Code subscription session on the host. The real-mode worker path is
# the same code, gated on the env var (FT-013 §Functional Specification).
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
cd "$REPO_ROOT"

cargo build --quiet --package decision-cli --bin dec
DEC="$REPO_ROOT/target/debug/dec"

# Ensure the python worker is set up. We invoke it through `uv run` so
# the harness picks up the project's pinned dependencies (pydantic +
# httpx) without polluting the system Python.
WORKER_DIR="$REPO_ROOT/workers/code-writer"
(cd "$WORKER_DIR" && uv sync --quiet)

WORKDIR="$(mktemp -d --tmpdir tc-008.XXXXXX)"
trap 'rm -rf "$WORKDIR"' EXIT
cd "$WORKDIR"

# --- 1. Bootstrap the orchestration store --------------------------------
"$DEC" init --template engineering-development >/dev/null

# --- 2. Stage a minimal product-cli graph slice -------------------------
# The harness will walk up from WORKDIR looking for `.product/`. We
# create an empty `.product/` here so the CodeChange artifact lands at
# `<WORKDIR>/.product/graph/code-changes.nq`.
mkdir -p .product/graph

# --- 3. Run dec implement against a feature id (the harness uses a -----
#       synthetic bundle when product-cli isn't on PATH for this TC). The
#       Worker subprocess is forced through `uv run` so the python deps
#       are available.
export CODE_WRITER_STUB=1
export CODE_WRITER_CMD="uv --project $WORKER_DIR run code-writer"

"$DEC" implement FT-008 >dec-out.txt

# --- 4. Parse minted IRIs ----------------------------------------------
SESSION_IRI=$(awk -F': +' '/^  Session:/ {print $2}' dec-out.txt)
CC_IRI=$(awk -F': +' '/^  CodeChange:/ {print $2}' dec-out.txt)
if [ -z "${SESSION_IRI:-}" ] || [ -z "${CC_IRI:-}" ]; then
  echo "TC-008 FAIL: could not parse minted IRIs from dec stdout:" >&2
  cat dec-out.txt >&2
  exit 1
fi

# --- 5. Session triples in dec graph -----------------------------------
SESSION_QUERY="PREFIX dec: <https://decision-cli.dev/ns#>
PREFIX prov: <http://www.w3.org/ns/prov#>
ASK {
  GRAPH ?g {
    <$SESSION_IRI> a dec:Session ;
                   a prov:Activity ;
                   prov:used ?bundle ;
                   prov:used ?model ;
                   dec:inStream <https://decision-cli.dev/ns/streams/engineering-development> ;
                   dec:featureId \"FT-008\" .
    ?bundle dec:contentHash ?bundleHash .
    ?model dec:modelVersion ?modelVersion .
  }
}"
ask_session=$("$DEC" _sparql --query "$SESSION_QUERY")
if [ "$ask_session" != "true" ]; then
  echo "TC-008 FAIL: Session PROV-O ASK returned $ask_session" >&2
  exit 1
fi

# --- 6. CodeChange in product graph with cross-store PROV link ---------
PRODUCT_CC_FILE=".product/graph/code-changes.nq"
if [ ! -f "$PRODUCT_CC_FILE" ]; then
  echo "TC-008 FAIL: product graph slice ($PRODUCT_CC_FILE) was not written" >&2
  exit 1
fi
expected_prov_line="<$CC_IRI> <http://www.w3.org/ns/prov#wasGeneratedBy> <$SESSION_IRI>"
if ! grep -F "$expected_prov_line" "$PRODUCT_CC_FILE" >/dev/null; then
  echo "TC-008 FAIL: product graph slice missing prov:wasGeneratedBy link" >&2
  echo "    expected substring: $expected_prov_line" >&2
  echo "    actual content:" >&2
  cat "$PRODUCT_CC_FILE" >&2
  exit 1
fi

# --- 7. At least one file written in the workspace ---------------------
WORKSPACE_DIR=".dec/workspace/FT-008"
if [ ! -d "$WORKSPACE_DIR" ]; then
  echo "TC-008 FAIL: workspace dir $WORKSPACE_DIR missing" >&2
  exit 1
fi
written_count=$(find "$WORKSPACE_DIR" -type f | wc -l)
if [ "$written_count" -lt 1 ]; then
  echo "TC-008 FAIL: no files written in workspace" >&2
  exit 1
fi

# --- 8. dec session show <iri> surfaces bundle hash + model version ----
if ! "$DEC" session show "$SESSION_IRI" >session-show.txt; then
  echo "TC-008 FAIL: dec session show exited non-zero" >&2
  exit 1
fi
if ! grep -q "Bundle hash:" session-show.txt; then
  echo "TC-008 FAIL: dec session show missing 'Bundle hash:'" >&2
  cat session-show.txt >&2
  exit 1
fi
if ! grep -q "Model version:" session-show.txt; then
  echo "TC-008 FAIL: dec session show missing 'Model version:'" >&2
  cat session-show.txt >&2
  exit 1
fi
if ! grep -q "Output:.*code-change" session-show.txt; then
  echo "TC-008 FAIL: dec session show missing CodeChange output ref" >&2
  cat session-show.txt >&2
  exit 1
fi

echo "TC-008 PASS"
