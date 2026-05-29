#!/usr/bin/env bash
# TC-252: Capability resolver no longer surfaces uniqueness errors after
# bootstrap with FT-118 lands (end-to-end regression test).

set -euo pipefail

TEMP_WORKDIR=$(mktemp -d)
trap 'rm -rf "$TEMP_WORKDIR"' EXIT

cd "$TEMP_WORKDIR"

# 1. Initialize a fresh workdir
dec init --template engineering-development

# 2. Create YAML files
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
  - capability_id: code-writer
    version: 2
    endpoint: anthropic
    model_identifier: claude-sonnet-4.5-20250610
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

# 3. Bootstrap v1 first for a test role (not one from init)
cat > config/role-bindings.yaml <<'YAML'
role_bindings:
  - role_id: test-bootstrap-role
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

# 4. Now add v7 as active (should deactivate v1 and activate v7)
cat > config/role-bindings.yaml <<'YAML'
role_bindings:
  - role_id: test-bootstrap-role
    version: 1
    default_capability: code-writer
    escalation_steps: []
    active: true
  - role_id: test-bootstrap-role
    version: 7
    default_capability: code-writer
    escalation_steps: []
    active: true
YAML

dec _bootstrap-catalog --capabilities config/capabilities.yaml --bindings config/role-bindings.yaml
if [ $? -ne 0 ]; then
    echo "FAIL: second bootstrap should succeed"
    exit 1
fi

# 5. Verify v7 is active, v1 is inactive
LIST_OUTPUT=$(dec _list-bindings --role test-bootstrap-role)
if ! echo "$LIST_OUTPUT" | grep -q "v7.*active"; then
    echo "FAIL: v7 should be active after bootstrap"
    echo "List output: $LIST_OUTPUT"
    exit 1
fi
if ! echo "$LIST_OUTPUT" | grep -q "v1.*inactive"; then
    echo "FAIL: v1 should be inactive after bootstrap"
    echo "List output: $LIST_OUTPUT"
    exit 1
fi

# 6. Verify exactly one active binding exists
ACTIVE_COUNT=$(echo "$LIST_OUTPUT" | grep -c " active " || true)
if [ "$ACTIVE_COUNT" -ne 1 ]; then
    echo "FAIL: exactly one active binding should exist after bootstrap (found $ACTIVE_COUNT)"
    echo "List output: $LIST_OUTPUT"
    exit 1
fi

echo "PASS: TC-252"
exit 0
