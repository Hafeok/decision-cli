#!/usr/bin/env bash
# TC-374 / FT-139 — positive case: add-judge-worker coherence audit
# accepts a synthetic fixture where all 5 cells agree on the input
# contract.
#
# Exit 0 — audit passed and one-line PASS summary printed.
# Exit 1 — audit rejected the consistent fixture (regression).

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
AUDIT_SCRIPT="$REPO_ROOT/scripts/checks/cluster-audit-add-judge-worker.py"

if [[ ! -x "$AUDIT_SCRIPT" ]]; then
  echo "TC-374 FAIL: audit script not executable at $AUDIT_SCRIPT" >&2
  exit 1
fi

FIXTURE_DIR="$(mktemp -d -t tc-374-add-judge-worker-positive-XXXXXX)"
trap 'rm -rf "$FIXTURE_DIR"' EXIT

cat > "$FIXTURE_DIR/capability_binding.nq" <<'NQ'
<https://decision-cli.dev/ns/capability/example-judge/v1> <https://decision-cli.dev/ns#endpoint> "scaleway" <https://decision-cli.dev/ns/graph/capability> .
<https://decision-cli.dev/ns/capability/example-judge/v1> <https://decision-cli.dev/ns#model_identifier> "qwen3-coder-30b-a3b-instruct" <https://decision-cli.dev/ns/graph/capability> .
NQ

cat > "$FIXTURE_DIR/pydantic_io_models.py" <<'PY'
from pydantic import BaseModel

class JudgeInput(BaseModel):
    feature_id: str
    proposed_artifact: str

class JudgeOutput(BaseModel):
    verdict: str
    reasoning: str
PY

cat > "$FIXTURE_DIR/system_prompt.md" <<'MD'
You are the example judge. Evaluate {{feature_id}} against {{proposed_artifact}}.
Emit a JSON verdict.
MD

cat > "$FIXTURE_DIR/agent_loop.py" <<'PY'
import litellm

def loop(payload, system_prompt, litellm_key, litellm_base_url):
    return litellm.completion(
        model=payload.model_id,
        messages=[
            {"role": "system", "content": system_prompt},
            {"role": "user", "content": f"{payload.feature_id} {payload.proposed_artifact}"},
        ],
        api_key=litellm_key,
        base_url=litellm_base_url,
    )
PY

cat > "$FIXTURE_DIR/unit_tests.py" <<'PY'
from .models import JudgeInput, JudgeOutput

def test_fixture_input():
    payload = JudgeInput(feature_id="FT-T374", proposed_artifact="example")
    assert payload.feature_id == "FT-T374"

def test_fixture_output():
    verdict = JudgeOutput(verdict="approved", reasoning="example")
    assert verdict.verdict == "approved"
PY

# Run the audit. We expect exit 0 + a PASS summary on stdout.
if ! OUTPUT="$(python3 "$AUDIT_SCRIPT" "$FIXTURE_DIR" 2>&1)"; then
  echo "TC-374 FAIL: audit rejected positive fixture" >&2
  echo "audit output:" >&2
  echo "$OUTPUT" >&2
  exit 1
fi

if ! grep -q "^PASS add-judge-worker" <<<"$OUTPUT"; then
  echo "TC-374 FAIL: audit exit 0 but missing PASS summary" >&2
  echo "audit output:" >&2
  echo "$OUTPUT" >&2
  exit 1
fi

echo "TC-374 PASS"
