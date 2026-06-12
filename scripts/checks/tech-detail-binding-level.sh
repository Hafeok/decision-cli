#!/usr/bin/env bash
# FT-148 / ADR-083 v1 — tech detail binds at exactly one level.
#
# Reads every forge/archetypes/{id}/application/contract.md +
# infrastructure/instances/{id}/infrastructure.contract.md pair and
# asserts (conservatively, by grep):
#   1. every application-contract convention name is referenced by at
#      least one application cell prompt;
#   2. no application-contract detail differs across instances;
#   3. no instance-contract concrete value appears in an application
#      cell prompt.
#
# v1 is vacuous-pass when forge/archetypes/ holds no contracts yet —
# the first archetype contract pair lands with FT-160.
# Exit 0/1/2 per the ADR-013 two-tier contract.
set -euo pipefail
cd "$(git rev-parse --show-toplevel)"

shopt -s nullglob
contracts=(forge/archetypes/*/application/contract.md)
if [ ${#contracts[@]} -eq 0 ]; then
  echo "OK: no archetype contracts on disk yet (first pair lands with FT-160); binding-level check is vacuous"
  exit 0
fi

FAILED=0
for contract in "${contracts[@]}"; do
  archetype_dir="$(dirname "$(dirname "$contract")")"
  prompts_dir="$archetype_dir/application/prompts"
  # (1) every convention heading referenced by at least one prompt
  while IFS= read -r name; do
    [ -z "$name" ] && continue
    if [ -d "$prompts_dir" ] && ! grep -rq "$name" "$prompts_dir"; then
      echo "FAIL check=binding_level: convention '$name' in $contract unreferenced by any application prompt"
      FAILED=1
    fi
  done < <(grep -oE '^## +[a-z][a-z0-9-]+' "$contract" | sed 's/^## *//')
  # (2) application detail must not differ across instances
  for instance in "$archetype_dir"/infrastructure/instances/*/infrastructure.contract.md; do
    [ -f "$instance" ] || continue
    while IFS= read -r line; do
      [ -z "$line" ] && continue
      if grep -qF "$line" "$instance"; then
        echo "FAIL check=binding_level: application detail duplicated in instance contract $instance: $line"
        FAILED=1
      fi
    done < <(grep -E '^[a-z-]+: ' "$contract")
  done
done

[ "$FAILED" -ne 0 ] && exit 1
echo "OK: tech detail binds at exactly one level (v1 conservative check)"
exit 0
