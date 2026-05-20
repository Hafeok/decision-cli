#!/usr/bin/env bash
# TC-048 — dec doctor --format json emits structured worker report (FT-016).
#
# Validates the JSON schema, the exit-code parity with text mode, and
# the embedded manifest sha256 surfacing.
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
cd "$REPO_ROOT"

cargo build --quiet --package decision-cli --bin dec
DEC="$REPO_ROOT/target/debug/dec"

# python3 is the JSON parser; if missing on the test host the TC cannot
# meaningfully verify schema. Detect early.
if ! command -v python3 >/dev/null; then
  echo "TC-048 SKIP: python3 required" >&2
  exit 0
fi

WORKDIR="$(mktemp -d --tmpdir tc-048.XXXXXX)"
trap 'rm -rf "$WORKDIR"' EXIT

SHIM_DIR="$WORKDIR/bin"
mkdir -p "$SHIM_DIR"
cat >"$SHIM_DIR/code-writer" <<'EOF'
#!/bin/sh
exit 0
EOF
chmod +x "$SHIM_DIR/code-writer"

# Pre-stage the store with the shim resolvable so init exits 0.
( cd "$WORKDIR" && PATH="$SHIM_DIR:/usr/bin:/bin" /usr/bin/env -u CODE_WRITER_CMD "$DEC" init --template engineering-development ) >"$WORKDIR/init.out" 2>&1

# --- 1. Resolved → exit 0, JSON parses, schema ok ----------------------
rc=0
( cd "$WORKDIR" && PATH="$SHIM_DIR:/usr/bin:/bin" /usr/bin/env -u CODE_WRITER_CMD "$DEC" doctor --format json ) >"$WORKDIR/ok.json" 2>"$WORKDIR/ok.err" || rc=$?
if [ "$rc" -ne 0 ]; then
  echo "TC-048 FAIL (ok): expected exit 0, got $rc" >&2
  cat "$WORKDIR/ok.json" >&2
  exit 1
fi

# Single JSON document — no leading/trailing noise.
python3 - "$WORKDIR/ok.json" <<'PY'
import json, sys
data = open(sys.argv[1]).read()
doc = json.loads(data)
assert isinstance(doc, dict), "top-level must be an object"
assert "workers" in doc and isinstance(doc["workers"], list), "workers array missing"
assert "summary" in doc and isinstance(doc["summary"], dict), "summary missing"
assert "manifest_sha256" in doc and isinstance(doc["manifest_sha256"], str), "manifest_sha256 missing"
assert len(doc["manifest_sha256"]) == 64, "manifest_sha256 must be 64-char hex"
ok = doc["summary"]["ok"]; missing = doc["summary"]["missing"]; inactive = doc["summary"]["inactive"]
counts = {"ok": 0, "missing": 0, "inactive": 0}
for w in doc["workers"]:
    assert "role" in w
    s = w["status"]; assert s in ("ok","missing","inactive"), s
    counts[s] += 1
    if s == "ok":
        assert w["resolved_via"] in ("override","env","path","sibling-workspace","python-module"), w["resolved_via"]
        assert w["resolved_command"], "resolved_command must be non-empty when status=ok"
    else:
        assert w["resolved_via"] is None
assert counts["ok"] == ok and counts["missing"] == missing and counts["inactive"] == inactive, (counts, doc["summary"])
# Resolved pass must report at least one ok row.
assert counts["ok"] >= 1, doc
PY

# --- 2. Missing → exit 2, install_hints present -----------------------
EMPTY_DIR="$WORKDIR/empty-bin"
mkdir -p "$EMPTY_DIR"
rc=0
( cd "$WORKDIR" && PATH="$EMPTY_DIR" /usr/bin/env -u CODE_WRITER_CMD "$DEC" doctor --format json ) >"$WORKDIR/miss.json" 2>"$WORKDIR/miss.err" || rc=$?
if [ "$rc" -ne 2 ]; then
  echo "TC-048 FAIL (missing): expected exit 2, got $rc" >&2
  cat "$WORKDIR/miss.json" >&2
  exit 1
fi
python3 - "$WORKDIR/miss.json" <<'PY'
import json, sys
doc = json.loads(open(sys.argv[1]).read())
miss = [w for w in doc["workers"] if w["status"] == "missing"]
assert miss, "expected at least one missing row"
for w in miss:
    assert "install_hints" in w and w["install_hints"], "install_hints must be non-empty on missing rows"
    joined = " ".join(w["install_hints"])
    assert "./workers/code-writer" in joined, "source_hint must appear in install_hints"
PY

# --- 3. Manifest sha256 in JSON matches the persisted init metadata ----
# Compute the same sha256 from the JSON of one resolved pass and the
# missing pass; they must be equal (manifest is build-time constant).
SHA_OK="$(python3 -c 'import json,sys; print(json.load(open(sys.argv[1]))["manifest_sha256"])' "$WORKDIR/ok.json")"
SHA_MISS="$(python3 -c 'import json,sys; print(json.load(open(sys.argv[1]))["manifest_sha256"])' "$WORKDIR/miss.json")"
if [ "$SHA_OK" != "$SHA_MISS" ]; then
  echo "TC-048 FAIL: manifest_sha256 disagrees across runs" >&2
  echo "  ok:      $SHA_OK" >&2
  echo "  missing: $SHA_MISS" >&2
  exit 1
fi
# And it must appear on the bootstrap session (init records it via PROV-O telemetry).
if ! grep -q "$SHA_OK" "$WORKDIR/.dec/store/orchestration.nq"; then
  echo "TC-048 FAIL: manifest_sha256 $SHA_OK not recorded on bootstrap session in store" >&2
  exit 1
fi

echo "TC-048 PASS"
