#!/usr/bin/env bash
# TC-094 — `dec verify env new --id <existing>` must refuse or upsert; never silently append.
#
# Bug discovered while dogfooding `dec verify` (2026-05-22): re-creating an
# environment with an existing id appends a second `dec:allowedOps` list to
# the store, after which `dec verify env list` aborts with
# "declares N dec:allowedOps heads".
#
# Acceptance: the second `env new` call must either
#   (refuse) exit non-zero with an error naming the existing id, OR
#   (upsert) exit 0 and leave the env in the store with exactly the new
#            allowed-ops list.
# In both cases `dec verify env list` must continue to succeed.
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
cd "$REPO_ROOT"

cargo build --quiet --package decision-cli --bin dec
DEC="$REPO_ROOT/target/debug/dec"

WORKDIR="$(mktemp -d --tmpdir tc-094.XXXXXX)"
trap 'rm -rf "$WORKDIR"' EXIT

cd "$WORKDIR"

# Bootstrap a store so verify env new can run.
"$DEC" init --template engineering-development >/dev/null 2>&1 || true

# First create: baseline ops.
"$DEC" verify env new \
  --id ENV-T \
  --type ephemeral-tempdir \
  --safety-class isolated \
  --allowed-ops shell,filesystem \
  >/dev/null

# Bug surface: duplicate-detection keys on the on-disk .ttl, not the
# store. Simulate disk/store drift (accidental rm, gitignore mismatch,
# manual cleanup) by removing the .ttl while the store still holds the
# artifact. The next `env new` must refuse or upsert — never silently
# append a second `allowedOps` list to the store.
rm -f .dec/verify/env/ENV-T.ttl

# Second create with same id, different ops.
rc=0
"$DEC" verify env new \
  --id ENV-T \
  --type ephemeral-tempdir \
  --safety-class isolated \
  --allowed-ops shell,filesystem,sparql-local \
  >/tmp/tc-094-second.out 2>&1 || rc=$?

if [ "$rc" -ne 0 ]; then
  # Refuse path: error must name ENV-T.
  if ! grep -q 'ENV-T' /tmp/tc-094-second.out; then
    echo "TC-094 FAIL: second env new refused but error did not name ENV-T" >&2
    cat /tmp/tc-094-second.out >&2
    exit 1
  fi
  echo "TC-094 PASS (refuse path)"
  exit 0
fi

# Upsert path: list must succeed and show ENV-T exactly once with the new ops.
list_rc=0
"$DEC" verify env list --format json >/tmp/tc-094-list.out 2>&1 || list_rc=$?
if [ "$list_rc" -ne 0 ]; then
  echo "TC-094 FAIL: list errored after second env new (silent-append bug)" >&2
  cat /tmp/tc-094-list.out >&2
  exit 1
fi

# Show must report deterministic single allowedOps including sparql-local.
show_rc=0
"$DEC" verify env show ENV-T --format json >/tmp/tc-094-show.out 2>&1 || show_rc=$?
if [ "$show_rc" -ne 0 ]; then
  echo "TC-094 FAIL: show errored on ENV-T after upsert" >&2
  cat /tmp/tc-094-show.out >&2
  exit 1
fi
if ! grep -q 'sparql-local' /tmp/tc-094-show.out; then
  echo "TC-094 FAIL: upsert did not replace allowed-ops (sparql-local missing in show output)" >&2
  cat /tmp/tc-094-show.out >&2
  exit 1
fi

echo "TC-094 PASS (upsert path)"
