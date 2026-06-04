#!/usr/bin/env bash
# TC-351 / FT-140 — positive: add-author-worker audit accepts a fixture
# with body_markdown + sections + no verdict.
set -euo pipefail
REPO_ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
AUDIT="$REPO_ROOT/scripts/checks/cluster-audit-add-author-worker.py"
FIX="$(mktemp -d -t tc-351-XXXXXX)"
trap 'rm -rf "$FIX"' EXIT

cat > "$FIX/capability_binding.nq" <<'NQ'
<https://decision-cli.dev/ns/capability/example/v1> <https://decision-cli.dev/ns#endpoint> "scaleway" <https://decision-cli.dev/ns/graph/capability> .
NQ

cat > "$FIX/pydantic_io_models.py" <<'PY'
from pydantic import BaseModel

class AuthorInput(BaseModel):
    brief: str
    feature_id: str

class AuthorOutput(BaseModel):
    body_markdown: str
    sections: dict[str, str]
PY

cat > "$FIX/system_prompt.md" <<'MD'
Author a spec for {{feature_id}} based on {{brief}}.
MD

cat > "$FIX/agent_loop.py" <<'PY'
import litellm
def loop(payload, sp, k, u):
    return litellm.completion(model=payload.model_id, messages=[], api_key=k, base_url=u)
PY

cat > "$FIX/unit_tests.py" <<'PY'
from .models import AuthorInput, AuthorOutput
def test_fixture():
    assert AuthorInput(brief="x", feature_id="FT-T").feature_id == "FT-T"
PY

OUT="$(python3 "$AUDIT" "$FIX" 2>&1)"
grep -q "^PASS add-author-worker" <<<"$OUT" || { echo "TC-351 FAIL: $OUT" >&2; exit 1; }
echo "TC-351 PASS"
