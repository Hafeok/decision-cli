#!/usr/bin/env bash
# TC-006 — `dec status` displays stream identity, definition source path,
#         content hash, terminal ValueAction URI, authorized goals, and
#         graph-store path matching what `dec init` recorded (FT-008 /
#         FT-012 / ADR-006 / ADR-004).
#
# Spec: .product/tests/TC-006-*.md
# Implements: FT-008 (init validation), FT-012 (dec status display).
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
cd "$REPO_ROOT"

# Build the binary (debug profile is fine for tests).
cargo build --quiet --package decision-cli --bin dec

DEC="$REPO_ROOT/target/debug/dec"
WORKDIR="$(mktemp -d --tmpdir tc-006.XXXXXX)"
trap 'rm -rf "$WORKDIR"' EXIT

cd "$WORKDIR"

# --- Setup: mirror TC-002's setup so the source path and hash are real. -----
mkdir -p streams
DEF_REL="./streams/decision-cli-development.ttl"
cat > "$DEF_REL" <<'EOF'
@prefix dec: <https://decision-cli.dev/ns#> .
@prefix va:  <https://decision-cli.dev/ns/value-actions/> .

<stream:decision-cli-development> a dec:ValueStream ;
    dec:name                "decision-cli-development" ;
    dec:title               "decision-cli Development" ;
    dec:description         "Value stream for shipping decision-cli features." ;
    dec:terminalValueAction va:shipped-feature ;
    dec:authorizedGoals     "ship" , "land" .
EOF

EXPECTED_HASH="$(sha256sum "$DEF_REL" | awk '{print $1}')"
EXPECTED_HASH_SHORT="${EXPECTED_HASH:0:12}"

if ! "$DEC" init --from "$DEF_REL" >/dev/null; then
  echo "TC-006 FAIL: dec init exited non-zero" >&2
  exit 1
fi

# --- Run `dec status` -------------------------------------------------------
if ! STATUS_OUT="$("$DEC" status)"; then
  echo "TC-006 FAIL: dec status exited non-zero" >&2
  echo "$STATUS_OUT" >&2
  exit 1
fi

fail() {
  echo "TC-006 FAIL: $1" >&2
  echo "--- dec status output ---" >&2
  echo "$STATUS_OUT" >&2
  echo "-------------------------" >&2
  exit 1
}

# 1. ValueStream name (from dec:name on the persisted stream).
case "$STATUS_OUT" in
  *decision-cli-development*) : ;;
  *) fail "stream name 'decision-cli-development' missing" ;;
esac

# 2. Definition source path (recorded via prov:wasDerivedFrom on the
#    bootstrap session in TC-002).
case "$STATUS_OUT" in
  *"./streams/decision-cli-development.ttl"*) : ;;
  *) fail "definition source path './streams/decision-cli-development.ttl' missing" ;;
esac

# 3. Content hash matches what was on disk (full or short-prefix form).
case "$STATUS_OUT" in
  *"$EXPECTED_HASH"*) : ;;
  *"$EXPECTED_HASH_SHORT"*) : ;;
  *) fail "expected SHA-256 hash $EXPECTED_HASH (prefix $EXPECTED_HASH_SHORT) missing" ;;
esac

# 4. Terminal ValueAction URI + bundled provenance label.
case "$STATUS_OUT" in
  *"va:shipped-feature"*) : ;;
  *) fail "prefixed ValueAction 'va:shipped-feature' missing" ;;
esac
case "$STATUS_OUT" in
  *bundled*) : ;;
  *) fail "bundled provenance label missing for va:shipped-feature" ;;
esac

# 4b. Ontology version (recorded on the bootstrap session per ADR-007).
case "$STATUS_OUT" in
  *"ontology v"*) : ;;
  *) fail "ontology version (e.g., 'ontology v0.1.0') missing" ;;
esac

# 5. Authorized goals list (must include both ship and land).
case "$STATUS_OUT" in
  *"ship"*"land"*) : ;;
  *"land"*"ship"*) : ;;
  *) fail "authorized goals 'ship, land' missing" ;;
esac

# 6. Graph-store path.
case "$STATUS_OUT" in
  *".dec/store"*) : ;;
  *) fail "graph store path '.dec/store' missing" ;;
esac

echo "TC-006 PASS"
