---
id: FT-TT-add-judge-worker
kind: task-type
phase: 5
status: in-progress
domains: [api, observability]
---

# Task Type: add-judge-worker

This task type automates the creation of Python judge workers with all the standard boilerplate.

## Recognition Signature

Features with `task_type: add-judge-worker` in their front-matter are recognized as belonging to this task type.

## Cell Cluster

The cluster consists of 5 cells that generate the full judge worker package in dependency order:

1. **`capability_binding`**
   - Artifact type: `capability_binding`
   - Prompt template: `templates/capability_binding.j2`
   - Model binding: `cap:judge-worker-binding`
   - Derived from: none

2. **`pydantic_io_models`**
   - Artifact type: `pydantic_io_models`
   - Prompt template: `templates/pydantic_io_models.j2`
   - Model binding: `cap:judge-worker-models`
   - Derived from: `capability_binding`

3. **`system_prompt`**
   - Artifact type: `system_prompt`
   - Prompt template: `templates/system_prompt.j2`
   - Model binding: `cap:judge-worker-prompt`
   - Derived from: `pydantic_io_models`

4. **`agent_loop`**
   - Artifact type: `agent_loop`
   - Prompt template: `templates/agent_loop.j2`
   - Model binding: `cap:judge-worker-loop`
   - Derived from: `pydantic_io_models`, `system_prompt`

5. **`unit_tests`**
   - Artifact type: `unit_tests`
   - Prompt template: `templates/unit_tests.j2`
   - Model binding: `cap:judge-worker-tests`
   - Derived from: `pydantic_io_models`, `agent_loop`

## Coherence Audit

The audit script ensures that all generated artifacts are consistent with each other:
- The agent loop calls LiteLLM with the correct model ID and endpoint
- The capability binding specifies valid model strings
- The pydantic models match what the agent loop reads
- The unit tests validate against the input model
- The system prompt references valid field names

Script: `scripts/checks/cluster-audit-add-judge-worker.py`
Timeout: 30 seconds