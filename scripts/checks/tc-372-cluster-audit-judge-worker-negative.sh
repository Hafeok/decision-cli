#!/usr/bin/env bash
# TC-372 / FT-139 — negative case: add-judge-worker coherence audit
# catches an agent_loop.py field reference absent from the pydantic
# input model. The load-bearing teeth test per ADR-080.
#
# Exit 0 — audit failed with the expected check identifier on stderr.
# Exit 1 — audit passed silently (REGRESSION: cluster pattern is now
#          silently weaker than the monolith) OR failed without the
#          expected check identifier.

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
AUDIT_SCRIPT="$REPO_ROOT/scripts/checks/cluster-audit-add-judge-worker.py"

if [[ ! -x "$AUDIT_SCRIPT" ]]; then
  echo "TC-372 FAIL: audit script not executable at $AUDIT_SCRIPT" >&2
  exit 1
fi

FIXTURE_DIR="$(mktemp -d -t tc-372-add-judge-worker-negative-XXXXXX)"
trap 'rm -rf "$FIXTURE_DIR"' EXIT

cat > "$FIXTURE_DIR/capability_binding.nq" <<'NQ'
<https://decision-cli.dev/ns/capability/example-judge/v1> <https://decision-cli.dev/ns#endpoint> "scaleway" <https://decision-cli.dev/ns/graph/capability> .
<https://decision-cli.dev/ns/capability/example-judge/v1> <https://decision-cli.dev/ns#model_identifier> "qwen3-coder-30b-a3b-instruct" <https://decision-cli.dev/ns/graph/capability> .
NQ

# Input model declares feature_id + tc_id; agent_loop will reference
# feature_spec_body which is ABSENT — the audit must catch this.
cat > "$FIXTURE_DIR/pydantic_io_models.py" <<'PY'
from pydantic import BaseModel

class JudgeInput(BaseModel):
    feature_id: str
    tc_id: str

class JudgeOutput(BaseModel):
    verdict: str
PY

cat > "$FIXTURE_DIR/system_prompt.md" <<'MD'
You are the example judge. Evaluate {{feature_id}} for TC {{tc_id}}.
MD

# agent_loop references payload.feature_spec_body — a field that does
# NOT exist on JudgeInput. The audit's agent_loop_field_coverage
# check should fire.
cat > "$FIXTURE_DIR/agent_loop.py" <<'PY'
import litellm

def loop(payload, system_prompt, litellm_key, litellm_base_url):
    return litellm.completion(
        model=payload.model_id,
        messages=[
            {"role": "system", "content": system_prompt},
            {"role": "user", "content": payload.feature_spec_body},
        ],
        api_key=litellm_key,
        base_url=litellm_base_url,
    )
PY

cat > "$FIXTURE_DIR/unit_tests.py" <<'PY'
from .models import JudgeInput

def test_fixture():
    payload = JudgeInput(feature_id="FT-T372", tc_id="TC-XYZ")
    assert payload.feature_id == "FT-T372"
PY

# Run the audit and capture exit + stderr.
set +e
STDOUT_FILE="$(mktemp)"
STDERR_FILE="$(mktemp)"
python3 "$AUDIT_SCRIPT" "$FIXTURE_DIR" >"$STDOUT_FILE" 2>"$STDERR_FILE"
EXIT_CODE=$?
set -e

trap 'rm -rf "$FIXTURE_DIR" "$STDOUT_FILE" "$STDERR_FILE"' EXIT

if [[ "$EXIT_CODE" -eq 0 ]]; then
  echo "TC-372 FAIL: audit accepted negative fixture (silent regression)" >&2
  echo "stdout: $(cat "$STDOUT_FILE")" >&2
  echo "stderr: $(cat "$STDERR_FILE")" >&2
  exit 1
fi

if ! grep -q "check=agent_loop_field_coverage" "$STDERR_FILE"; then
  echo "TC-372 FAIL: audit failed but did not surface check=agent_loop_field_coverage" >&2
  echo "stderr:" >&2
  cat "$STDERR_FILE" >&2
  exit 1
fi

if ! grep -q "feature_spec_body" "$STDERR_FILE"; then
  echo "TC-372 FAIL: audit error did not name the offending field" >&2
  echo "stderr:" >&2
  cat "$STDERR_FILE" >&2
  exit 1
fi

echo "TC-372 PASS"
