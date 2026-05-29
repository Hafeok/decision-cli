#!/usr/bin/env bash
# TC-250: dec _activate-binding errors out when activating would create
# duplicate-active state (scenario test for FT-118).

set -euo pipefail

TEMP_WORKDIR=$(mktemp -d)
trap 'rm -rf "$TEMP_WORKDIR"' EXIT

cd "$TEMP_WORKDIR"

# 1. Initialize a workdir
dec init --template engineering-development

# 2. Create YAML files with two bindings (v1 and v7) for test-role
mkdir -p config
cat > config/capabilities.yaml <<'YAML'
capabilities:
  - capability_id: code-writer
    version: 1
    endpoint: anthropic
    model_identifier: claude-sonnet-4-20250514
    tier: 1
    context_window: 200000
    max_output: 16000
    supports_vision: true
    supports_tool_calling: true
    cost_input_per_m: "3.00"
    cost_output_per_m: "15.00"
    cost_currency: USD
    status: active
YAML

# Bootstrap v1 first (will be active)
cat > config/role-bindings.yaml <<'YAML'
role_bindings:
  - role_id: test-role
    version: 1
    default_capability: code-writer
    escalation_steps: []
    active: true
YAML

dec _bootstrap-catalog --capabilities config/capabilities.yaml --bindings config/role-bindings.yaml
if [ $? -ne 0 ]; then
    echo "FAIL: initial bootstrap should succeed"
    exit 1
fi

# 3. Add v7 to the YAML (inactive initially)
cat > config/role-bindings.yaml <<'YAML'
role_bindings:
  - role_id: test-role
    version: 1
    default_capability: code-writer
    escalation_steps: []
    active: true
  - role_id: test-role
    version: 7
    default_capability: code-writer
    escalation_steps: []
    active: false
YAML

dec _bootstrap-catalog --capabilities config/capabilities.yaml --bindings config/role-bindings.yaml
if [ $? -ne 0 ]; then
    echo "FAIL: bootstrap with v7 inactive should succeed"
    exit 1
fi

# 4. Activate v7 — should succeed and deactivate v1
dec _activate-binding --role test-role --version 7
if [ $? -ne 0 ]; then
    echo "FAIL: _activate-binding should succeed"
    exit 1
fi

# 5. Verify: v7 active, v1 inactive
LIST_OUTPUT=$(dec _list-bindings --role test-role)
if ! echo "$LIST_OUTPUT" | grep -q "v7.*active"; then
    echo "FAIL: v7 should be active after activation"
    exit 1
fi
if ! echo "$LIST_OUTPUT" | grep -q "v1.*inactive"; then
    echo "FAIL: v1 should be inactive after v7 activation"
    exit 1
fi

# 6. Try to activate a nonexistent version — should fail
if dec _activate-binding --role test-role --version 99 2>/dev/null; then
    echo "FAIL: _activate-binding should fail for nonexistent binding"
    exit 1
fi

echo "PASS: TC-250"
exit 0
