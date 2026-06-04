#!/usr/bin/env bash
# TC-352 / FT-140 — discriminator teeth: Output has `verdict` instead of
# `body_markdown`; audit must fail with `output_is_draft_not_verdict`
# and hint at add-judge-worker.
set -euo pipefail
REPO_ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
AUDIT="$REPO_ROOT/scripts/checks/cluster-audit-add-author-worker.py"
FIX="$(mktemp -d -t tc-352-XXXXXX)"
trap 'rm -rf "$FIX"' EXIT

cat > "$FIX/capability_binding.nq" <<'NQ'
<https://decision-cli.dev/ns/capability/example/v1> <https://decision-cli.dev/ns#endpoint> "scaleway" <https://decision-cli.dev/ns/graph/capability> .
NQ

cat > "$FIX/pydantic_io_models.py" <<'PY'
from pydantic import BaseModel

class AuthorInput(BaseModel):
    brief: str

# WRONG: this is a judge output shape, not an author output.
class AuthorOutput(BaseModel):
    verdict: str
    reasoning: str
PY

cat > "$FIX/system_prompt.md" <<'MD'
Stub.
MD

cat > "$FIX/agent_loop.py" <<'PY'
import litellm
def loop(p, sp, k, u):
    return litellm.completion(model=p.model_id, messages=[], api_key=k, base_url=u)
PY

cat > "$FIX/unit_tests.py" <<'PY'
from .models import AuthorOutput
PY

set +e
ERR="$(python3 "$AUDIT" "$FIX" 2>&1 >/dev/null)"
CODE=$?
set -e
[[ "$CODE" -eq 0 ]] && { echo "TC-352 FAIL: audit accepted negative fixture (silent regression)" >&2; exit 1; }
grep -q "check=output_is_draft_not_verdict" <<<"$ERR" \
  || { echo "TC-352 FAIL: missing discriminator check id; got: $ERR" >&2; exit 1; }
grep -q "add-judge-worker" <<<"$ERR" \
  || { echo "TC-352 FAIL: discriminator hint absent; got: $ERR" >&2; exit 1; }
echo "TC-352 PASS"
