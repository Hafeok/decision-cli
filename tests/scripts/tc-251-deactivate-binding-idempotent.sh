#!/usr/bin/env bash
# TC-251: dec _deactivate-binding flips active to false idempotently and
# preserves binding history (scenario test for FT-118).

set -euo pipefail

TEMP_WORKDIR=$(mktemp -d)
trap 'rm -rf "$TEMP_WORKDIR"' EXIT

cd "$TEMP_WORKDIR"

# 1. Initialize a workdir
dec init --template engineering-development

# 2. Create YAML and bootstrap an active binding
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

cat > config/role-bindings.yaml <<'YAML'
role_bindings:
  - role_id: test-role-2
    version: 1
    default_capability: code-writer
    escalation_steps: []
    active: true
YAML

dec _bootstrap-catalog --capabilities config/capabilities.yaml --bindings config/role-bindings.yaml
if [ $? -ne 0 ]; then
    echo "FAIL: bootstrap should succeed"
    exit 1
fi

# 3. Deactivate the binding
dec _deactivate-binding --role test-role-2 --version 1
if [ $? -ne 0 ]; then
    echo "FAIL: _deactivate-binding should succeed"
    exit 1
fi

# 4. Verify it's inactive
LIST_OUTPUT=$(dec _list-bindings --role test-role-2)
if ! echo "$LIST_OUTPUT" | grep -q "v1.*inactive"; then
    echo "FAIL: v1 should be inactive after deactivation"
    exit 1
fi

# 5. Verify history is preserved (default_capability should still be code-writer)
if ! echo "$LIST_OUTPUT" | grep -q "default=.*code-writer"; then
    echo "FAIL: binding history should be preserved after deactivation"
    exit 1
fi

# 6. Deactivate again (idempotent check)
DEACTIVATE_OUTPUT=$(dec _deactivate-binding --role test-role-2 --version 1)
if [ $? -ne 0 ]; then
    echo "FAIL: second _deactivate-binding should succeed (idempotent)"
    exit 1
fi
if ! echo "$DEACTIVATE_OUTPUT" | grep -q "no-op"; then
    echo "FAIL: second deactivation should report no-op"
    exit 1
fi

# 7. Try to deactivate nonexistent binding
if dec _deactivate-binding --role test-role-2 --version 99 2>/dev/null; then
    echo "FAIL: _deactivate-binding should fail for nonexistent binding"
    exit 1
fi

echo "PASS: TC-251"
exit 0
