#!/usr/bin/env bash
# TC-047 — dec doctor exit code mirrors worker resolution outcome (FT-016).
#
# Authoritative on-demand audit. Exit code 0 on all-OK, non-zero on
# any missing role. Read-only: store + workspace must not be mutated.
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
cd "$REPO_ROOT"

cargo build --quiet --package decision-cli --bin dec
DEC="$REPO_ROOT/target/debug/dec"

WORKDIR="$(mktemp -d --tmpdir tc-047.XXXXXX)"
trap 'rm -rf "$WORKDIR"' EXIT

# Pre-stage: provide a code-writer shim so init succeeds with exit 0.
SHIM_DIR="$WORKDIR/bin"
mkdir -p "$SHIM_DIR"
cat >"$SHIM_DIR/code-writer" <<'EOF'
#!/bin/sh
exit 0
EOF
chmod +x "$SHIM_DIR/code-writer"

# Bootstrap the store with the shim resolvable.
( cd "$WORKDIR" && PATH="$SHIM_DIR:/usr/bin:/bin" /usr/bin/env -u CODE_WRITER_CMD "$DEC" init --template engineering-development ) >"$WORKDIR/init.out" 2>&1

NQ="$WORKDIR/.dec/store/orchestration.nq"
SHA_BEFORE="$(sha256sum "$NQ" | awk '{print $1}')"
MTIME_BEFORE="$(stat -c %Y "$NQ")"

# --- 1. All resolved → exit 0 -------------------------------------------
rc=0
( cd "$WORKDIR" && PATH="$SHIM_DIR:/usr/bin:/bin" /usr/bin/env -u CODE_WRITER_CMD "$DEC" doctor ) >"$WORKDIR/doc-ok.out" 2>&1 || rc=$?
if [ "$rc" -ne 0 ]; then
  echo "TC-047 FAIL (ok): expected exit 0, got $rc" >&2
  cat "$WORKDIR/doc-ok.out" >&2
  exit 1
fi
if ! grep -Eq '^  code-writer\s+OK\s+\S+' "$WORKDIR/doc-ok.out"; then
  echo "TC-047 FAIL (ok): expected '  code-writer  OK  <path>' row" >&2
  cat "$WORKDIR/doc-ok.out" >&2
  exit 1
fi

# --- 2. Any missing → non-zero (status 2) -------------------------------
EMPTY_DIR="$WORKDIR/empty-bin"
mkdir -p "$EMPTY_DIR"
rc=0
( cd "$WORKDIR" && PATH="$EMPTY_DIR" /usr/bin/env -u CODE_WRITER_CMD "$DEC" doctor ) >"$WORKDIR/doc-miss.out" 2>&1 || rc=$?
if [ "$rc" -ne 2 ]; then
  echo "TC-047 FAIL (missing): expected exit 2, got $rc" >&2
  cat "$WORKDIR/doc-miss.out" >&2
  exit 1
fi
if ! grep -Eq '^  code-writer\s+MISSING' "$WORKDIR/doc-miss.out"; then
  echo "TC-047 FAIL (missing): expected MISSING row" >&2
  cat "$WORKDIR/doc-miss.out" >&2
  exit 1
fi
if ! grep -q 'uv tool install' "$WORKDIR/doc-miss.out"; then
  echo "TC-047 FAIL (missing): expected install hint" >&2
  cat "$WORKDIR/doc-miss.out" >&2
  exit 1
fi

# --- 3. --role <role> filters; unknown role errors out -----------------
rc=0
( cd "$WORKDIR" && PATH="$SHIM_DIR:/usr/bin:/bin" /usr/bin/env -u CODE_WRITER_CMD "$DEC" doctor --role code-writer ) >"$WORKDIR/doc-role.out" 2>&1 || rc=$?
if [ "$rc" -ne 0 ]; then
  echo "TC-047 FAIL (role-ok): expected exit 0, got $rc" >&2
  cat "$WORKDIR/doc-role.out" >&2
  exit 1
fi
# Only one OK row should appear (and no other roles).
rows="$(grep -cE '^  [a-z]' "$WORKDIR/doc-role.out" || true)"
if [ "$rows" -ne 1 ]; then
  echo "TC-047 FAIL (role-ok): expected exactly 1 row, got $rows" >&2
  cat "$WORKDIR/doc-role.out" >&2
  exit 1
fi

rc=0
( cd "$WORKDIR" && PATH="$SHIM_DIR:/usr/bin:/bin" /usr/bin/env -u CODE_WRITER_CMD "$DEC" doctor --role nope ) >"$WORKDIR/doc-unknown.out" 2>&1 || rc=$?
if [ "$rc" -eq 0 ]; then
  echo "TC-047 FAIL (unknown role): expected non-zero exit, got 0" >&2
  cat "$WORKDIR/doc-unknown.out" >&2
  exit 1
fi

# --- 4. Read-only: store sha + mtime unchanged --------------------------
SHA_AFTER="$(sha256sum "$NQ" | awk '{print $1}')"
MTIME_AFTER="$(stat -c %Y "$NQ")"
if [ "$SHA_BEFORE" != "$SHA_AFTER" ]; then
  echo "TC-047 FAIL: store sha256 changed across dec doctor invocations" >&2
  exit 1
fi
if [ "$MTIME_BEFORE" != "$MTIME_AFTER" ]; then
  echo "TC-047 FAIL: store mtime changed across dec doctor invocations" >&2
  exit 1
fi

# --- 5. Read-only: workspace tree unchanged (init artifacts only) ------
# A naive check: no extra files appear under .dec/ after the doctor runs.
post_files="$(find "$WORKDIR/.dec" -type f | wc -l)"
expected_files=3   # orchestration.nq + definition.ttl + init-metadata.json
if [ "$post_files" -ne "$expected_files" ]; then
  echo "TC-047 FAIL: expected $expected_files files under .dec/, found $post_files" >&2
  find "$WORKDIR/.dec" -type f >&2
  exit 1
fi

echo "TC-047 PASS"
