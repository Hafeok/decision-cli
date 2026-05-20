#!/usr/bin/env bash
# TC-046 — dec init prints worker preflight after bootstrap (FT-016).
#
# Two passes:
#   (a) code-writer resolvable on $PATH → exit 0, OK row.
#   (b) code-writer unresolvable        → advisory exit 2, MISSING row + hints.
# In both cases, the orchestration store must exist post-init.
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
cd "$REPO_ROOT"

cargo build --quiet --package decision-cli --bin dec
DEC="$REPO_ROOT/target/debug/dec"

ORIG_PATH="$PATH"

PASS_A="$(mktemp -d --tmpdir tc-046a.XXXXXX)"
PASS_B="$(mktemp -d --tmpdir tc-046b.XXXXXX)"
trap 'rm -rf "$PASS_A" "$PASS_B"' EXIT

# --- Pass (a): code-writer present on $PATH --------------------------------
SHIM_DIR_A="$PASS_A/bin"
mkdir -p "$SHIM_DIR_A"
cat >"$SHIM_DIR_A/code-writer" <<'EOF'
#!/bin/sh
exit 0
EOF
chmod +x "$SHIM_DIR_A/code-writer"

OUT_A="$PASS_A/init.out"
rc=0
( cd "$PASS_A" && PATH="$SHIM_DIR_A:/usr/bin:/bin" /usr/bin/env -u CODE_WRITER_CMD "$DEC" init --template engineering-development ) >"$OUT_A" 2>&1 || rc=$?
if [ "$rc" -ne 0 ]; then
  echo "TC-046 FAIL (ok): expected exit 0, got $rc" >&2
  cat "$OUT_A" >&2
  exit 1
fi
if ! grep -q '^Worker preflight:' "$OUT_A"; then
  echo "TC-046 FAIL (ok): no 'Worker preflight:' header in stdout" >&2
  cat "$OUT_A" >&2
  exit 1
fi
if ! grep -Eq '^  code-writer\s+OK\s+\S+' "$OUT_A"; then
  echo "TC-046 FAIL (ok): expected '  code-writer  OK  <path>' row" >&2
  cat "$OUT_A" >&2
  exit 1
fi
if [ ! -f "$PASS_A/.dec/store/orchestration.nq" ]; then
  echo "TC-046 FAIL (ok): orchestration.nq missing" >&2
  exit 1
fi

# --- Pass (b): code-writer not resolvable ---------------------------------
EMPTY_DIR="$PASS_B/empty-bin"
mkdir -p "$EMPTY_DIR"
OUT_B="$PASS_B/init.out"
rc=0
# Use only the empty directory as PATH so `code-writer` cannot be found
# and `python3 -c "import code_writer.main"` cannot run (no python3
# either). CODE_WRITER_CMD is unset via `env -u`.
( cd "$PASS_B" && PATH="$EMPTY_DIR" /usr/bin/env -u CODE_WRITER_CMD "$DEC" init --template engineering-development ) >"$OUT_B" 2>&1 || rc=$?
if [ "$rc" -ne 2 ]; then
  echo "TC-046 FAIL (missing): expected advisory exit 2, got $rc" >&2
  cat "$OUT_B" >&2
  exit 1
fi
if ! grep -q '^Worker preflight:' "$OUT_B"; then
  echo "TC-046 FAIL (missing): no 'Worker preflight:' header" >&2
  cat "$OUT_B" >&2
  exit 1
fi
if ! grep -Eq '^  code-writer\s+MISSING' "$OUT_B"; then
  echo "TC-046 FAIL (missing): expected MISSING row" >&2
  cat "$OUT_B" >&2
  exit 1
fi
if ! grep -q 'uv tool install ./workers/code-writer' "$OUT_B"; then
  echo "TC-046 FAIL (missing): expected install hint 'uv tool install ./workers/code-writer'" >&2
  cat "$OUT_B" >&2
  exit 1
fi
if ! grep -q 'CODE_WRITER_CMD' "$OUT_B"; then
  echo "TC-046 FAIL (missing): expected CODE_WRITER_CMD override hint" >&2
  cat "$OUT_B" >&2
  exit 1
fi
if [ ! -f "$PASS_B/.dec/store/orchestration.nq" ]; then
  echo "TC-046 FAIL (missing): orchestration.nq missing (bootstrap rolled back?)" >&2
  exit 1
fi
if ! grep -q 'session/init-001' "$PASS_B/.dec/store/orchestration.nq"; then
  echo "TC-046 FAIL (missing): bootstrap session not in store" >&2
  exit 1
fi

# Restore PATH (defensive).
PATH="$ORIG_PATH"
export PATH

echo "TC-046 PASS"
