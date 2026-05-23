#!/usr/bin/env bash
# TC-118 — code-writer subprocess runner contains no hardcoded model
# identifiers outside test fixtures (FT-066, extends TC-113).
#
# Spec: .product/tests/TC-118-code-writer-subprocess-runner-contains-no-hardcode.md
#
# Invariant: every model literal seen by `claude -p` (via the env vars
# computed in `_claude_env_for`) must originate from `payload.model_id`.
# Hardcoded provider model strings (claude-sonnet, claude-opus, qwen3,
# devstral, gpt-oss, mistral-small, ANTHROPIC_MODEL=…) MUST NOT appear
# in the runner / env-routing modules outside of test fixtures.
#
# Allowlist:
#   * tests/        (legitimately pin specific models per scenario)
#   * __pycache__/  (generated)
#   * .venv/        (third-party)
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
cd "$REPO_ROOT"

# The runner-level files under audit. The whole worker tree must obey
# the rule, but this TC specifically pins the spawn-side modules called
# out in FT-066 §Invariants.
TARGET_GLOBS=(
  "workers/code-writer/src/code_writer/_subprocess_runner.py"
  "workers/code-writer/src/code_writer/env_routing.py"
)

for target in "${TARGET_GLOBS[@]}"; do
  if [[ ! -f "$target" ]]; then
    echo "TC-118: required runner file missing: $target" >&2
    exit 1
  fi
done

# Patterns that would indicate a hardcoded provider model identifier.
# Each is grep-friendly extended-regex.
PATTERNS=(
  'claude-(sonnet|opus|haiku)-[0-9]'
  'qwen3-?[a-z0-9-]+-instruct'
  'devstral-[0-9]'
  'gpt-oss-[0-9]+b'
  'mistral-small-[0-9]'
)

violations=0
for target in "${TARGET_GLOBS[@]}"; do
  for pattern in "${PATTERNS[@]}"; do
    # Find lines matching the pattern that are NOT pure comments
    # referencing the model as documentation. Comment lines starting
    # with `#` followed by whitespace are accepted as commentary;
    # everything else is a violation.
    if matches="$(grep -nE "$pattern" "$target" || true)"; then
      if [[ -n "$matches" ]]; then
        while IFS= read -r line; do
          content="${line#*:}"
          stripped="${content#"${content%%[![:space:]]*}"}"  # left-trim
          if [[ "$stripped" == \#* ]]; then
            continue
          fi
          echo "TC-118 VIOLATION: hardcoded model identifier in $target" >&2
          echo "  $line" >&2
          violations=$((violations + 1))
        done <<< "$matches"
      fi
    fi
  done
done

# Also assert the runner exposes the FT-066 env-routing helper rather
# than building env overlays inline.
if ! grep -q 'claude_env_for' "workers/code-writer/src/code_writer/_subprocess_runner.py"; then
  echo "TC-118 VIOLATION: _subprocess_runner.py does not invoke claude_env_for" >&2
  echo "  env-routing must flow through the shared helper, not inline ifs" >&2
  violations=$((violations + 1))
fi

# The env-routing module MUST source model identifiers from the payload,
# not from constants. A bare assignment to a model-shaped string is a
# regression. The only mention of a specific model id we allow is in
# the *docstring* of the module, never in executable code.
if grep -nE '^[^#]*MODEL.*=.*"(claude|qwen|devstral|gpt-oss|mistral)' \
    "workers/code-writer/src/code_writer/env_routing.py" >&2 ; then
  echo "TC-118 VIOLATION: env_routing.py declares a constant model id" >&2
  violations=$((violations + 1))
fi

if (( violations > 0 )); then
  echo "TC-118: $violations violation(s) found" >&2
  exit 1
fi

echo "TC-118 OK: no hardcoded model identifiers in code-writer runner"
exit 0
