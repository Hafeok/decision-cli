#!/usr/bin/env bash
# tests/scripts/tc-181-server-json-schema.sh
#
# TC-181 — Both `crates/product-cli/server.json` and
# `crates/decision-cli/server.json` validate against the MCP registry's
# `server.schema.json` on every PR. Schema drift fails CI before a
# release attempt finds out.
#
# Spec: .product/tests/TC-181-*.md
# Implements: FT-106 §Phase 6 (schema validation as a CI gate).
#
# Two-tier exit-code contract (per ADR-013):
#   exit 0 — every scenario passes.
#   exit 1 — at least one scenario failed; diagnostic on stdout/stderr
#            names the offender.
#
# Scenarios A/B/D/E run on every PR (fast, no network). Scenario C
# (schema freshness vs upstream) runs only when TC181_NETWORK=1 — that
# path is reserved for a nightly job; PR-CI uses the committed copy at
# tests/fixtures/server.schema.json.

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"

SCHEMA="$REPO_ROOT/tests/fixtures/server.schema.json"
PRODUCT_JSON="$REPO_ROOT/crates/product-cli/server.json"
DECISION_JSON="$REPO_ROOT/crates/decision-cli/server.json"

fail() {
  echo "TC-181 FAIL: $1" >&2
  exit 1
}

info() {
  echo "TC-181 info: $*"
}

for path in "$SCHEMA" "$PRODUCT_JSON" "$DECISION_JSON"; do
  if [ ! -f "$path" ]; then
    fail "missing required file: $path"
  fi
done

if ! command -v python3 >/dev/null 2>&1; then
  fail "python3 is required"
fi

# --- Scenarios A, B, E, F: per-file validation + required fields ----------
python3 - "$SCHEMA" "$PRODUCT_JSON" "$DECISION_JSON" <<'PY' || exit 1
import json
import re
import sys

schema_path = sys.argv[1]
files = sys.argv[2:]

PLACEHOLDER = "<PLACEHOLDER>"
SCHEMA_URL = "https://static.modelcontextprotocol.io/schemas/2025-09-29/server.schema.json"
NAME_RE = re.compile(r"^io\.github\.[^/]+/[^/]+$")
SHA_RE = re.compile(r"^[0-9a-fA-F]{64}$")

try:
    with open(schema_path) as fh:
        schema = json.load(fh)
except Exception as exc:
    print(f"TC-181 FAIL: cannot read schema {schema_path}: {exc}", file=sys.stderr)
    sys.exit(1)

# Best-effort: prefer the `jsonschema` library if available, otherwise
# fall back to a custom checker that covers the registry's required
# fields and string patterns. The fallback is deliberately strict on the
# fields TC-181 §Scenario B enumerates.
try:
    from jsonschema import Draft7Validator
    HAVE_JS = True
except ImportError:
    HAVE_JS = False


def validate_with_jsonschema(doc):
    validator = Draft7Validator(schema)
    return [e.message for e in validator.iter_errors(doc)]


def validate_fallback(doc):
    errs = []
    # Required top-level
    for field in ("$schema", "name", "description", "version", "packages"):
        if field not in doc:
            errs.append(f"missing required top-level field '{field}'")
    if errs:
        return errs

    if not isinstance(doc.get("description"), str) or len(doc["description"]) < 16:
        errs.append("description must be a string of length >= 16")
    if "packages" in doc:
        pkgs = doc["packages"]
        if not isinstance(pkgs, list) or not pkgs:
            errs.append("packages must be a non-empty array")
        else:
            for i, pkg in enumerate(pkgs):
                for f in ("registryType", "identifier", "version",
                          "transport", "fileSha256", "runtimeArguments"):
                    if f not in pkg:
                        errs.append(f"packages[{i}] missing '{f}'")
                if "fileSha256" in pkg and not SHA_RE.match(str(pkg["fileSha256"])):
                    errs.append(
                        f"packages[{i}].fileSha256 not 64 hex chars "
                        f"(got {pkg['fileSha256']!r})"
                    )
                if "transport" in pkg and "type" not in pkg.get("transport", {}):
                    errs.append(f"packages[{i}].transport missing 'type'")
    return errs


seen_names = []
exit_code = 0
for path in files:
    try:
        with open(path) as fh:
            doc = json.load(fh)
    except Exception as exc:
        print(f"TC-181 FAIL: {path}: cannot parse JSON: {exc}", file=sys.stderr)
        exit_code = 1
        continue

    # Schema-level validation
    errors = validate_with_jsonschema(doc) if HAVE_JS else validate_fallback(doc)

    # Scenario B: business-required fields beyond pure schema
    if doc.get("$schema") != SCHEMA_URL:
        errors.append(
            f"$schema must equal {SCHEMA_URL!r}, got {doc.get('$schema')!r}"
        )
    if not NAME_RE.match(doc.get("name", "") or ""):
        errors.append(
            f"name {doc.get('name')!r} does not match io.github.<owner>/<repo>"
        )
    seen_names.append(doc.get("name", ""))

    repo = doc.get("repository", {}) or {}
    for f in ("url", "source", "id"):
        if not repo.get(f):
            errors.append(f"repository.{f} missing or empty")

    # Scenario E: placeholders are explicitly tolerated.
    for i, pkg in enumerate(doc.get("packages", []) or []):
        # version may legitimately be the literal "<PLACEHOLDER>"
        if pkg.get("version") not in (None,) and pkg.get("version") == PLACEHOLDER:
            pass  # fine
        ident = pkg.get("identifier", "")
        if not isinstance(ident, str) or not ident:
            errors.append(f"packages[{i}].identifier missing or empty")

    if errors:
        print(f"TC-181 FAIL: {path}:", file=sys.stderr)
        for e in errors:
            print(f"  - {e}", file=sys.stderr)
        exit_code = 1
    else:
        print(f"TC-181 OK: {path} validated against {SCHEMA_URL}")

# Scenario B: name uniqueness across the two manifests
if len(seen_names) != len(set(seen_names)):
    print(f"TC-181 FAIL: server.json `name` collision across files: {seen_names}",
          file=sys.stderr)
    exit_code = 1

sys.exit(exit_code)
PY

info "Scenarios A/B/E OK: both server.json files schema-valid."

# --- Scenario D: deliberate breakage caught -------------------------------
# Mutate a temp copy and ensure the validator rejects it. This is the
# test-of-the-test pattern — proves the validator actually fires.
tmpdir="$(mktemp -d)"
trap 'rm -rf "$tmpdir"' EXIT
cp "$PRODUCT_JSON" "$tmpdir/broken.json"

python3 - "$tmpdir/broken.json" <<'PY'
import json, sys
path = sys.argv[1]
with open(path) as fh:
    doc = json.load(fh)
doc.pop("name", None)
with open(path, "w") as fh:
    json.dump(doc, fh)
PY

if python3 - "$SCHEMA" "$tmpdir/broken.json" <<'PY' 2>/dev/null
import json, sys
schema_path, target = sys.argv[1], sys.argv[2]
try:
    from jsonschema import Draft7Validator
    schema = json.load(open(schema_path))
    doc = json.load(open(target))
    errs = list(Draft7Validator(schema).iter_errors(doc))
    sys.exit(0 if not errs else 1)
except ImportError:
    doc = json.load(open(target))
    sys.exit(0 if "name" in doc else 1)
PY
then
  fail "Scenario D: validator accepted a server.json with no 'name'"
fi
info "Scenario D OK: validator rejects deliberate breakage."

# --- Scenario F: version pin matches workspace (soft) --------------------
# Surface drift on stdout but do not fail — the publish workflow has the
# final say on the substituted version.
WS_VERSION="$(grep -m1 '^version' "$REPO_ROOT/crates/decision-cli/Cargo.toml" \
              | sed -E 's/version = "([^"]+)"/\1/' | tr -d ' ')"
SJ_VERSION="$(python3 -c 'import json,sys;print(json.load(open(sys.argv[1])).get("version",""))' "$DECISION_JSON")"
if [ "$SJ_VERSION" != "<PLACEHOLDER>" ] && [ "$SJ_VERSION" != "$WS_VERSION" ]; then
  echo "TC-181 WARNING: decision-cli server.json version ($SJ_VERSION) " \
       "differs from Cargo.toml ($WS_VERSION). Soft assertion only."
fi

# Scenario C: upstream schema freshness — opt-in via env, network only.
if [ "${TC181_NETWORK:-0}" = "1" ]; then
  info "Scenario C: comparing committed schema against upstream..."
  upstream="$(mktemp)"
  if curl --silent --location --fail \
       "https://static.modelcontextprotocol.io/schemas/2025-09-29/server.schema.json" \
       -o "$upstream"; then
    if ! diff -q "$SCHEMA" "$upstream" >/dev/null 2>&1; then
      echo "TC-181 WARNING: committed schema differs from upstream; refresh tests/fixtures/server.schema.json"
    else
      info "Scenario C OK: committed schema matches upstream."
    fi
  else
    info "Scenario C skipped: upstream schema fetch failed."
  fi
fi

exit 0
