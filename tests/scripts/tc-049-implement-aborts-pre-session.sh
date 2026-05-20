#!/usr/bin/env bash
# TC-049 — dec implement aborts before session open on worker resolution failure.
#
# Validates that the pre-session preflight gate (FT-016) prevents
# `dec implement` from opening a session, writing a bundle, or spawning
# the worker subprocess when the resolution chain cannot find a worker.
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
cd "$REPO_ROOT"

cargo build --quiet --package decision-cli --bin dec
DEC="$REPO_ROOT/target/debug/dec"

WORKDIR="$(mktemp -d --tmpdir tc-049.XXXXXX)"
trap 'rm -rf "$WORKDIR"' EXIT

# Pre-stage: shim code-writer + init the store. We want a fixture with
# the bootstrap session in place so the post-run "no new session" check
# is meaningful.
SHIM_DIR="$WORKDIR/bin"
mkdir -p "$SHIM_DIR"
cat >"$SHIM_DIR/code-writer" <<'EOF'
#!/bin/sh
exit 0
EOF
chmod +x "$SHIM_DIR/code-writer"

( cd "$WORKDIR" && PATH="$SHIM_DIR:/usr/bin:/bin" /usr/bin/env -u CODE_WRITER_CMD "$DEC" init --template engineering-development ) >"$WORKDIR/init.out" 2>&1

NQ="$WORKDIR/.dec/store/orchestration.nq"
SHA_BEFORE="$(sha256sum "$NQ" | awk '{print $1}')"
SESSION_COUNT_BEFORE="$(grep -c 'a <http://www.w3.org/ns/prov#Activity>' "$NQ" || echo 0)"

# --- Unresolvable run ---------------------------------------------------
EMPTY_DIR="$WORKDIR/empty-bin"
mkdir -p "$EMPTY_DIR"
rc=0
( cd "$WORKDIR" && PATH="$EMPTY_DIR" /usr/bin/env -u CODE_WRITER_CMD "$DEC" implement FT-XXX ) >"$WORKDIR/imp.out" 2>"$WORKDIR/imp.err" || rc=$?
if [ "$rc" -eq 0 ]; then
  echo "TC-049 FAIL: expected non-zero exit (worker missing), got 0" >&2
  cat "$WORKDIR/imp.err" >&2
  exit 1
fi

# Diagnostic must include install hints, not the legacy "exited with exit status".
if ! grep -q 'uv tool install ./workers/code-writer' "$WORKDIR/imp.err"; then
  echo "TC-049 FAIL: stderr missing 'uv tool install ./workers/code-writer'" >&2
  cat "$WORKDIR/imp.err" >&2
  exit 1
fi
if ! grep -q 'CODE_WRITER_CMD' "$WORKDIR/imp.err"; then
  echo "TC-049 FAIL: stderr missing CODE_WRITER_CMD override hint" >&2
  cat "$WORKDIR/imp.err" >&2
  exit 1
fi
if grep -q 'code-writer worker exited with exit status' "$WORKDIR/imp.err"; then
  echo "TC-049 FAIL: legacy 'worker exited with exit status' surfaced — preflight bypassed" >&2
  cat "$WORKDIR/imp.err" >&2
  exit 1
fi

# Graph integrity: no new sessions, no store mutation.
SHA_AFTER="$(sha256sum "$NQ" | awk '{print $1}')"
if [ "$SHA_BEFORE" != "$SHA_AFTER" ]; then
  echo "TC-049 FAIL: store sha256 changed across the aborted run" >&2
  exit 1
fi
SESSION_COUNT_AFTER="$(grep -c 'a <http://www.w3.org/ns/prov#Activity>' "$NQ" || echo 0)"
if [ "$SESSION_COUNT_BEFORE" != "$SESSION_COUNT_AFTER" ]; then
  echo "TC-049 FAIL: prov:Activity count changed: before=$SESSION_COUNT_BEFORE after=$SESSION_COUNT_AFTER" >&2
  exit 1
fi

# No workspace tree under .dec/workspace/FT-XXX should have been created
# (the workspace dir is built AFTER preflight).
if [ -d "$WORKDIR/.dec/workspace/FT-XXX" ]; then
  echo "TC-049 FAIL: .dec/workspace/FT-XXX was created — preflight ran AFTER workspace" >&2
  exit 1
fi

echo "TC-049 PASS"
