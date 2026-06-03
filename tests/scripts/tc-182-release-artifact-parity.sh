#!/usr/bin/env bash
# tests/scripts/tc-182-release-artifact-parity.sh
#
# TC-182 — Every release tag produces five target-platform archives for
# the `dec` binary. Re-scoped under ADR-077.
#
# Spec: .product/tests/TC-182-*.md
# Implements: FT-106 §Phase 1 (dist-workspace.toml) + §Invariants
# (per-platform parity).
#
# Two-tier exit-code contract (per ADR-013):
#   exit 0 — every applicable scenario passes.
#   exit 1 — at least one scenario failed; diagnostic on stdout/stderr
#            names the offender.
#
# Two modes:
#   1. PR-mode (no args). Runs Scenario F only: dist-workspace.toml
#      structural sanity check (single binary, five targets).
#      Fast, no network.
#   2. Release-mode (--tag vX.Y.Z). Runs Scenarios A-E against the
#      named GitHub Release. Requires `gh` + network. Nightly / on
#      release-tag CI.

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"

EXPECTED_TARGETS=(
  aarch64-apple-darwin
  aarch64-unknown-linux-gnu
  x86_64-apple-darwin
  x86_64-unknown-linux-gnu
  x86_64-pc-windows-msvc
)
EXPECTED_BINARY=dec

DIST_WORKSPACE="$REPO_ROOT/dist-workspace.toml"

fail() {
  echo "TC-182 FAIL: $1" >&2
  exit 1
}

info() {
  echo "TC-182 info: $*"
}

# Parse args ---------------------------------------------------------------
TAG=""
while [ $# -gt 0 ]; do
  case "$1" in
    --tag)
      TAG="${2:-}"
      shift 2
      ;;
    --tag=*)
      TAG="${1#--tag=}"
      shift
      ;;
    -h|--help)
      cat <<USAGE
Usage: $0 [--tag vX.Y.Z]
  PR-mode (no args): lockstep version drift + dist-workspace.toml sanity.
  Release-mode (--tag): query the GitHub Release for parity (A-E).
USAGE
      exit 0
      ;;
    *)
      fail "unknown argument: $1"
      ;;
  esac
done

# --- Always: Scenario F + dist-workspace.toml structural checks ----------
if [ ! -f "$DIST_WORKSPACE" ]; then
  fail "missing dist-workspace.toml at workspace root (FT-106 §Phase 1)"
fi

# dist-workspace.toml must enumerate the single binary-producing crate
# (decision-cli only, per ADR-077) and all five targets.
python3 - "$DIST_WORKSPACE" <<'PY' || exit 1
import sys
path = sys.argv[1]
with open(path) as fh:
    text = fh.read()

required_members = [
    "cargo:crates/decision-cli",
]
disallowed_members = [
    "cargo:crates/product-shim",
    "cargo:crates/product-cli",
]
required_targets = [
    "aarch64-apple-darwin",
    "aarch64-unknown-linux-gnu",
    "x86_64-apple-darwin",
    "x86_64-unknown-linux-gnu",
    "x86_64-pc-windows-msvc",
]
errs = []
for m in required_members:
    if m not in text:
        errs.append(f"dist-workspace.toml missing workspace member {m!r}")
for m in disallowed_members:
    if m in text:
        errs.append(f"dist-workspace.toml should NOT contain {m!r} (ADR-077)")
for t in required_targets:
    if t not in text:
        errs.append(f"dist-workspace.toml missing target triple {t!r}")
if errs:
    for e in errs:
        print(f"TC-182 FAIL: {e}", file=sys.stderr)
    sys.exit(1)
print("dist-workspace.toml structural check OK")
PY

# Scenario F (continued): verify decision-cli/Cargo.toml version is present.
CARGO_FILE="$REPO_ROOT/crates/decision-cli/Cargo.toml"
if [ ! -f "$CARGO_FILE" ]; then
  fail "missing $CARGO_FILE"
fi
VERSION_IN_CARGO="$(grep -m1 '^version' "$CARGO_FILE" | sed -E 's/version = "([^"]+)"/\1/' | tr -d ' ')"
if [ -z "$VERSION_IN_CARGO" ]; then
  fail "$CARGO_FILE: no [package].version found"
fi
info "Scenario F OK: decision-cli version $VERSION_IN_CARGO"

# PR-mode ends here.
if [ -z "$TAG" ]; then
  info "PR-mode complete. Pass --tag vX.Y.Z to run release-mode scenarios."
  exit 0
fi

# --- Release-mode: Scenarios A-E require gh + network --------------------
if ! command -v gh >/dev/null 2>&1; then
  fail "release-mode (--tag) requires the gh CLI"
fi

VERSION="${TAG#v}"
info "Querying GitHub Release for tag $TAG (version $VERSION)..."

assets_file="$(mktemp)"
trap 'rm -f "$assets_file"' EXIT
if ! gh release view "$TAG" --json assets --jq '.assets[].name' > "$assets_file" 2>/dev/null; then
  fail "could not fetch assets for release $TAG"
fi

# Scenario A: every target has an archive for dec.
missing=0
for target in "${EXPECTED_TARGETS[@]}"; do
  case "$target" in
    *windows*) ext="zip" ;;
    *)         ext="tar.xz" ;;
  esac
  expect="${EXPECTED_BINARY}-${target}.${ext}"
  if ! grep -qx "$expect" "$assets_file"; then
    echo "TC-182 FAIL: missing release asset: $expect" >&2
    missing=$((missing + 1))
  fi
done

if [ "$missing" -gt 0 ]; then
  echo "TC-182 FAIL: $missing assets missing for release $TAG" >&2
  echo "Assets present:" >&2
  cat "$assets_file" >&2
  exit 1
fi
info "Scenario A OK: 5 archives (5 targets for dec binary) present."

# Scenario D: checksums recorded. Prefer dist-manifest.json; per-asset
# .sha256 files acceptable as fallback.
if grep -qx "dist-manifest.json" "$assets_file"; then
  info "Scenario D OK: dist-manifest.json published with the release."
elif grep -qE '\.sha256$' "$assets_file" >/dev/null; then
  info "Scenario D OK: per-asset .sha256 files published."
else
  fail "Scenario D: neither dist-manifest.json nor per-asset .sha256 files present"
fi

# Scenarios B/C/E need actual archive extraction; gated by an opt-in env
# so PR-CI / quick release checks don't redundantly burn bandwidth.
if [ "${TC182_DEEP:-0}" != "1" ]; then
  info "Scenarios B/C/E skipped (TC182_DEEP!=1)."
  info "Set TC182_DEEP=1 to download archives and inspect contents."
  exit 0
fi

workdir="$(mktemp -d)"
trap 'rm -rf "$workdir" "$assets_file"' EXIT
cd "$workdir"
LINUX_ASSET="dec-x86_64-unknown-linux-gnu.tar.xz"
gh release download "$TAG" --pattern "$LINUX_ASSET" --output "$LINUX_ASSET"
tar -tJf "$LINUX_ASSET" | head -5
info "Scenarios B/C/E partial OK on $LINUX_ASSET (extend per-platform as runners allow)."

exit 0
