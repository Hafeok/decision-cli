#!/usr/bin/env bash
# TC-096 — `dec verify env list` must degrade gracefully on a corrupt env.
#
# When a single VerificationEnvironment in the store is malformed (today:
# two `dec:allowedOps` heads from the TC-094 silent-append bug; tomorrow:
# any schema drift), `dec verify env list` must continue to list the
# healthy envs and report a structured error row for the broken one —
# instead of aborting the whole command.
#
# Corruption setup: we exercise the TC-094 bug path to manufacture a
# multi-headed env (the natural way to produce this state today). When the
# TC-094 fix lands, this script will need a different corruption seed
# (e.g. raw store write); the test's invariant (list must not abort)
# stays the same.
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
cd "$REPO_ROOT"

cargo build --quiet --package decision-cli --bin dec
DEC="$REPO_ROOT/target/debug/dec"

WORKDIR="$(mktemp -d --tmpdir tc-096.XXXXXX)"
trap 'rm -rf "$WORKDIR"' EXIT

cd "$WORKDIR"

"$DEC" init --template engineering-development >/dev/null 2>&1 || true

# Two healthy envs that must still appear in the listing.
"$DEC" verify env new --id ENV-HEALTHY-A --type ephemeral-tempdir --safety-class isolated \
  --allowed-ops shell,filesystem >/dev/null
"$DEC" verify env new --id ENV-HEALTHY-B --type ephemeral-tempdir --safety-class isolated \
  --allowed-ops shell,filesystem,sparql-local >/dev/null

# Manufacture a corrupt env via the TC-094 disk/store drift path: create
# the env, remove the .ttl while the store still holds it, then re-create
# — which today silently appends a second `allowedOps` list to the store.
# If TC-094 lands first and `env new` becomes upsert/refuse, this block
# stops producing corruption; at that point this script must be updated
# to seed corruption a different way (write raw quads into
# `.dec/store/orchestration.nq` directly). The list-resilience invariant
# under test does not depend on how the corruption is produced.
"$DEC" verify env new --id ENV-BROKEN --type ephemeral-tempdir --safety-class isolated \
  --allowed-ops shell,filesystem >/dev/null
rm -f .dec/verify/env/ENV-BROKEN.ttl
"$DEC" verify env new --id ENV-BROKEN --type ephemeral-tempdir --safety-class isolated \
  --allowed-ops shell,filesystem,sparql-local >/dev/null 2>&1 || true

# The invariant: list must not abort. It may return 0 or a documented
# partial-success exit, but stderr/stdout must include all three env ids.
rc=0
"$DEC" verify env list --format json >/tmp/tc-096-list.out 2>/tmp/tc-096-list.err || rc=$?

if [ "$rc" -ne 0 ] && [ "$rc" -ne 2 ]; then
  echo "TC-096 FAIL: list aborted with unexpected exit $rc (expected 0 or partial-success 2)" >&2
  echo "--- stdout ---" >&2; cat /tmp/tc-096-list.out >&2
  echo "--- stderr ---" >&2; cat /tmp/tc-096-list.err >&2
  exit 1
fi

combined="$(cat /tmp/tc-096-list.out /tmp/tc-096-list.err)"
for id in ENV-HEALTHY-A ENV-HEALTHY-B ENV-BROKEN; do
  if ! grep -q "$id" <<<"$combined"; then
    echo "TC-096 FAIL: $id missing from list output" >&2
    echo "--- stdout ---" >&2; cat /tmp/tc-096-list.out >&2
    echo "--- stderr ---" >&2; cat /tmp/tc-096-list.err >&2
    exit 1
  fi
done

# The broken env's row must carry an error marker so an operator can
# triage. Either a structured JSON field or a textual marker is fine;
# the matching list below is the union of acceptable markers.
if ! grep -Eq 'allowedOps|corrupt|error|multiple|heads' <<<"$combined"; then
  echo "TC-096 FAIL: no error marker present for ENV-BROKEN" >&2
  echo "--- stdout ---" >&2; cat /tmp/tc-096-list.out >&2
  echo "--- stderr ---" >&2; cat /tmp/tc-096-list.err >&2
  exit 1
fi

echo "TC-096 PASS"
