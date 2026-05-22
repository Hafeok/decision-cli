#!/usr/bin/env bash
# TC-095 — `dec verify graph new --id <existing>` must refuse or upsert; never silently append.
#
# Same structural bug as TC-094 but on the graph creation path. Re-creating
# a graph with the same id silently appends a second `dec:environment`
# binding (and other triples) into the store, after which `dec verify
# graph list` / `show` produce nondeterministic output.
#
# Acceptance: the second `graph new` call must either
#   (refuse) exit non-zero with an error naming the existing id, OR
#   (upsert) exit 0 and leave the graph in the store bound to exactly the
#            new environment.
# In both cases `dec verify graph list` and `show` must continue to succeed.
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
cd "$REPO_ROOT"

cargo build --quiet --package decision-cli --bin dec
DEC="$REPO_ROOT/target/debug/dec"

WORKDIR="$(mktemp -d --tmpdir tc-095.XXXXXX)"
trap 'rm -rf "$WORKDIR"' EXIT

cd "$WORKDIR"

"$DEC" init --template engineering-development >/dev/null 2>&1 || true

# `dec verify graph new --verifies FT-008` resolves against
# `<workdir>/.product/features/`; seed a minimal stub so the reference
# resolves in this tempdir.
mkdir -p .product/features
cat >.product/features/FT-008-stub.md <<'EOF'
---
id: FT-008
title: stub for TC-095
status: complete
phase: 1
---
stub
EOF

# Two clean envs to switch between.
"$DEC" verify env new --id ENV-A --type ephemeral-tempdir --safety-class isolated \
  --allowed-ops shell,filesystem,sparql-local >/dev/null
"$DEC" verify env new --id ENV-B --type ephemeral-tempdir --safety-class isolated \
  --allowed-ops shell,filesystem,sparql-local >/dev/null

# First create: VG-042 bound to ENV-A.
"$DEC" verify graph new --id VG-042 --verifies FT-008 --environment ENV-A >/dev/null

# Bug surface: duplicate-detection keys on the on-disk .ttl, not the
# store. Removing the file while the store still holds the graph
# exercises the silent-append path.
rm -f .dec/verify/graph/VG-042.ttl

# Second create with same id, different env.
rc=0
"$DEC" verify graph new --id VG-042 --verifies FT-008 --environment ENV-B \
  >/tmp/tc-095-second.out 2>&1 || rc=$?

if [ "$rc" -ne 0 ]; then
  if ! grep -q 'VG-042' /tmp/tc-095-second.out; then
    echo "TC-095 FAIL: second graph new refused but error did not name VG-042" >&2
    cat /tmp/tc-095-second.out >&2
    exit 1
  fi
  echo "TC-095 PASS (refuse path)"
  exit 0
fi

# Upsert path: list must succeed AND show VG-042 exactly once. The
# silent-append bug surfaces here: `graph show` only renders one
# binding (nondeterministic, currently the newer), so it can falsely
# look correct — but `graph list` emits one row per stored
# `dec:environment` triple, so a duplicated binding shows up as VG-042
# appearing twice.
list_rc=0
"$DEC" verify graph list --format json >/tmp/tc-095-list.out 2>&1 || list_rc=$?
if [ "$list_rc" -ne 0 ]; then
  echo "TC-095 FAIL: list errored after second graph new" >&2
  cat /tmp/tc-095-list.out >&2
  exit 1
fi
count="$(grep -c '"id": "VG-042"' /tmp/tc-095-list.out || true)"
if [ "$count" -ne 1 ]; then
  echo "TC-095 FAIL: list returned VG-042 ${count}× (expected 1; silent-append bug)" >&2
  cat /tmp/tc-095-list.out >&2
  exit 1
fi
if ! grep -q 'ENV-B' /tmp/tc-095-list.out; then
  echo "TC-095 FAIL: upsert did not point VG-042 at ENV-B" >&2
  cat /tmp/tc-095-list.out >&2
  exit 1
fi
if grep -q 'ENV-A' /tmp/tc-095-list.out; then
  echo "TC-095 FAIL: upsert did not remove the previous ENV-A binding" >&2
  cat /tmp/tc-095-list.out >&2
  exit 1
fi

echo "TC-095 PASS (upsert path)"
