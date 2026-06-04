#!/usr/bin/env bash
# Coherence audit for the `extend-planner-classifier` TaskType (FT-143 /
# ADR-080).
#
# Discriminator: touches planner.rs / inspect_dor.rs — NOT seeds.rs
# (that's extend-role-catalog-seed's territory).
#
# Checks:
#   1. type_consistency — inspector_trait_method's return type matches
#      inspector_production_impl's return type.
#   2. state_hash_signal — state_hash_update.rs references the new
#      signal name in a hasher.write() call.
#   3. no_seed_touch — no .rs file references seeds.rs (discriminator
#      vs extend-role-catalog-seed).
#
# Exit 0 / 1 / 2 per ADR-013 contract.

set -euo pipefail

die() {
  local check="$1" detail="$2"
  echo "FAIL check=${check}: ${detail}" >&2
  exit 1
}

[[ $# -eq 1 ]] || { echo "usage: $0 <fixture_dir>" >&2; exit 2; }
FIX="$1"
[[ -d "$FIX" ]] || { echo "fixture $FIX is not a directory" >&2; exit 2; }

TRAIT_FILE="$FIX/inspector_trait_method.rs"
PROD_FILE="$FIX/inspector_production_impl.rs"
HASH_FILE="$FIX/state_hash_update.rs"

# Check 1: type consistency.
if [[ -f "$TRAIT_FILE" && -f "$PROD_FILE" ]]; then
  trait_ret=$(grep -oE 'Result<[^,>]+(, [^>]+)?>' "$TRAIT_FILE" | head -1 || true)
  prod_ret=$(grep -oE 'Result<[^,>]+(, [^>]+)?>' "$PROD_FILE" | head -1 || true)
  if [[ -n "$trait_ret" && -n "$prod_ret" && "$trait_ret" != "$prod_ret" ]]; then
    die "type_mismatch" \
      "inspector_trait_method returns ${trait_ret} but inspector_production_impl returns ${prod_ret}"
  fi
fi

# Check 2: state hash signal.
if [[ -f "$HASH_FILE" ]]; then
  if ! grep -q 'hasher\.write\|hash(' "$HASH_FILE"; then
    die "state_hash_missing" \
      "state_hash_update.rs does not appear to fold any signal into the hasher"
  fi
  # Specifically check the new signal name (signal_name file dropped by
  # the TC fixture).
  if [[ -f "$FIX/_signal_name" ]]; then
    signal="$(cat "$FIX/_signal_name")"
    if ! grep -q "$signal" "$HASH_FILE"; then
      die "state_hash_missing" \
        "state_hash_update.rs does not fold the new signal name '${signal}' into the hash"
    fi
  fi
fi

# Check 3: discriminator — no seeds.rs touch.
if grep -rq "seeds\.rs\|role_catalog/seeds" "$FIX" --include="*.rs" 2>/dev/null; then
  die "no_seed_touch" \
    "cluster references seeds.rs — did you mean extend-role-catalog-seed?"
fi

echo "PASS extend-planner-classifier (3 checks passed)"
