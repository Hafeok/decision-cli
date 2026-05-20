#!/usr/bin/env bash
# scripts/checks/vertical-slice-layout.sh
#
# Validates the FT-018 / ADR-016 vertical-slice layout exit criterion
# (TC-020). Asserts that:
#
#   1. The expected `core/` and `features/` directory shape is present.
#   2. The legacy flat modules at `crates/decision-cli/src/` are gone.
#   3. `crates/decision-cli/src/main.rs` is at most 250 lines.
#   4. `cargo build --workspace` and `cargo test --workspace` succeed.
#
# Exit 0: layout matches and the workspace is green.
# Exit 1: any assertion fails. Diagnostic lines on stdout name the
#         offending entry, line count, or step.
set -euo pipefail

if REPO_ROOT="$(git rev-parse --show-toplevel 2>/dev/null)"; then
  cd "$REPO_ROOT"
fi

MAIN_LIMIT=${MAIN_RS_LINE_LIMIT_FT018:-250}

FAILED=0

src=crates/decision-cli/src

assert_file() {
  local path="$1" label="$2"
  if [ ! -f "$path" ]; then
    echo "ERROR: missing $label ($path)"
    FAILED=1
  fi
}

assert_module() {
  # `<base>/<name>.rs` OR `<base>/<name>/mod.rs`.
  local base="$1" name="$2" label="$3"
  if [ ! -f "$base/$name.rs" ] && [ ! -f "$base/$name/mod.rs" ]; then
    echo "ERROR: missing $label ($base/$name.rs or $base/$name/mod.rs)"
    FAILED=1
  fi
}

assert_feature_mod() {
  # Feature must be a directory with a `mod.rs`.
  local name="$1"
  if [ ! -f "$src/features/$name/mod.rs" ]; then
    echo "ERROR: missing features::$name ($src/features/$name/mod.rs)"
    FAILED=1
  fi
}

# 1. lib.rs / main.rs / core/mod.rs / features/mod.rs.
assert_file "$src/lib.rs" "crate lib.rs"
assert_file "$src/main.rs" "crate main.rs"
assert_file "$src/core/mod.rs" "core/mod.rs"
assert_file "$src/features/mod.rs" "features/mod.rs"

# 2. core submodules.
for m in ontology vocab bundled scope stream_writer store sparql; do
  assert_module "$src/core" "$m" "core::$m"
done

# 3. feature submodules.
for f in init implement health events session_inspect finalize; do
  assert_feature_mod "$f"
done

# 4. legacy flat modules MUST NOT be at the crate root.
for legacy in init.rs implement.rs health.rs events.rs session_inspect.rs \
              finalize.rs scope.rs stream_writer.rs bundled.rs ontology.rs \
              vocab.rs; do
  if [ -e "$src/$legacy" ]; then
    echo "ERROR: legacy flat module still at $src/$legacy"
    FAILED=1
  fi
done

# 5. main.rs line cap.
if [ -f "$src/main.rs" ]; then
  count="$(wc -l < "$src/main.rs" | tr -d ' ')"
  if [ "$count" -gt "$MAIN_LIMIT" ]; then
    echo "ERROR: $src/main.rs has $count lines (FT-018 limit: $MAIN_LIMIT)"
    FAILED=1
  fi
fi

if [ "$FAILED" -ne 0 ]; then
  echo ""
  echo "Layout assertions failed; skipping cargo build/test."
  exit 1
fi

# 6. Workspace builds and tests pass.
if ! cargo build --workspace --quiet 2>&1; then
  echo "ERROR: cargo build --workspace failed"
  exit 1
fi

if ! cargo test --workspace --quiet 2>&1; then
  echo "ERROR: cargo test --workspace failed"
  exit 1
fi

echo "OK: ADR-016 vertical-slice layout intact (main.rs ${count}/${MAIN_LIMIT} lines)"
exit 0
