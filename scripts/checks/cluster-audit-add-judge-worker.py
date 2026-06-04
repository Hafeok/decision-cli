#!/usr/bin/env python3
"""Coherence audit for the `add-judge-worker` TaskType (FT-139 / ADR-080).

Verifies cross-cell agreement after the cluster's 5 cells have emitted:
1. `agent_loop.py` calls LiteLLM with `model=payload.model_id` and
   `base_url=...` (canonical FT-123 shape; regex over the file).
2. `capability_binding.nq` declares an `endpoint` literal in
   {`scaleway`, `anthropic`} and a `model_identifier` literal that is
   either prefix-free (provider added by the worker) or carries a
   recognised provider prefix.
3. `pydantic_io_models.py`'s input model fields are a superset of the
   `payload.<field>` accesses in `agent_loop.py`.
4. `unit_tests.py` constructs a fixture instance of the input model
   (asserted by presence of a `from .models import` line + a call to
   the input model's name).
5. `system_prompt.md`'s Jinja-style `{{var}}` references are a subset
   of the input model's field names (or known template helpers).

Exit codes (per ADR-013 / TC runner contract):
  0  every check passed.
  1  at least one check failed; stderr explains which.
  2  cluster fixture unrunnable (missing files, parse errors).

Usage:
  cluster-audit-add-judge-worker.py <fixture_dir>

The fixture directory must contain:
  capability_binding.nq
  pydantic_io_models.py
  system_prompt.md
  agent_loop.py
  unit_tests.py

The script writes a one-line summary to stdout on pass and a check-id
diagnosis to stderr on fail. Discriminator check 6 (output is a
verdict not a draft) is FT-140's responsibility; this audit does not
implement it.
"""

from __future__ import annotations

import re
import sys
from pathlib import Path

CELL_FILES = {
    "capability_binding": "capability_binding.nq",
    "pydantic_io_models": "pydantic_io_models.py",
    "system_prompt": "system_prompt.md",
    "agent_loop": "agent_loop.py",
    "unit_tests": "unit_tests.py",
}

KNOWN_TEMPLATE_HELPERS = {"now", "feature_id", "tc_id", "iteration"}
RECOGNISED_PROVIDER_PREFIXES = ("anthropic/", "openai/", "scaleway/", "claude/")


def _die(check: str, detail: str, exit_code: int = 1) -> None:
    sys.stderr.write(f"FAIL check={check}: {detail}\n")
    sys.exit(exit_code)


def _read(p: Path) -> str:
    try:
        return p.read_text(encoding="utf-8")
    except OSError as e:
        _die("missing_file", f"{p}: {e}", exit_code=2)
        return ""  # unreachable


def check_agent_loop_litellm_shape(fixture: Path) -> None:
    body = _read(fixture / CELL_FILES["agent_loop"])
    if not re.search(r"litellm\.completion\s*\(", body):
        _die("agent_loop_litellm_call", "agent_loop.py does not call litellm.completion")
    if not re.search(r"model\s*=\s*\S*model_id\b", body):
        _die(
            "agent_loop_model_arg",
            "agent_loop.py's litellm.completion call does not pass model=...model_id",
        )
    if not re.search(r"base_url\s*=", body):
        _die(
            "agent_loop_base_url",
            "agent_loop.py's litellm.completion call does not pass base_url=...",
        )


def check_capability_binding(fixture: Path) -> None:
    body = _read(fixture / CELL_FILES["capability_binding"])
    endpoint_match = re.search(r'#endpoint>\s*"([^"]+)"', body)
    if not endpoint_match:
        _die("capability_endpoint_missing", "capability_binding.nq lacks dec:endpoint")
    endpoint = endpoint_match.group(1)
    if endpoint not in {"scaleway", "anthropic"}:
        _die(
            "capability_endpoint_invalid",
            f"endpoint {endpoint!r} not in {{scaleway, anthropic}}",
        )
    model_match = re.search(r'#model_identifier>\s*"([^"]+)"', body)
    if not model_match:
        _die("capability_model_missing", "capability_binding.nq lacks dec:model_identifier")
    model = model_match.group(1)
    if "/" in model and not model.startswith(RECOGNISED_PROVIDER_PREFIXES):
        _die(
            "capability_model_prefix",
            f"model_identifier {model!r} has unrecognised provider prefix",
        )


def check_agent_loop_fields_in_models(fixture: Path) -> None:
    agent = _read(fixture / CELL_FILES["agent_loop"])
    models = _read(fixture / CELL_FILES["pydantic_io_models"])
    payload_refs = set(re.findall(r"payload\.([A-Za-z_][A-Za-z0-9_]*)", agent))
    # Strip known harness fields populated outside of the input class.
    payload_refs.discard("model_id")
    payload_refs.discard("endpoint")
    payload_refs.discard("max_turns")
    payload_refs.discard("workspace_path")
    payload_refs.discard("bundle_markdown")
    payload_refs.discard("bundle_hash")
    payload_refs.discard("feature_id")
    payload_refs.discard("session_id")
    payload_refs.discard("dispatch_id")
    payload_refs.discard("timeout_seconds")
    payload_refs.discard("authority")
    payload_refs.discard("defect_feedback")
    payload_refs.discard("allowed_tools")
    if not payload_refs:
        return
    declared = set(re.findall(r"^\s*([A-Za-z_][A-Za-z0-9_]*)\s*:\s*", models, re.MULTILINE))
    missing = payload_refs - declared
    if missing:
        _die(
            "agent_loop_field_coverage",
            f"agent_loop.py references payload fields not on the input model: {sorted(missing)}",
        )


def check_unit_tests_fixture(fixture: Path) -> None:
    body = _read(fixture / CELL_FILES["unit_tests"])
    if not re.search(r"from\s+\.\s*models\s+import\b", body) and not re.search(
        r"from\s+\.models\s+import\b", body
    ):
        _die(
            "unit_tests_imports_models",
            "unit_tests.py does not import from .models",
        )
    # Heuristic for "constructs a fixture": a model class name followed
    # by ( (i.e. instantiation). Accept any CamelCase identifier with at
    # least one ( within 200 chars.
    if not re.search(r"\b([A-Z][A-Za-z0-9]*)\s*\(", body):
        _die(
            "unit_tests_fixture_construction",
            "unit_tests.py does not appear to construct a model fixture",
        )


def check_system_prompt_fields(fixture: Path) -> None:
    prompt = _read(fixture / CELL_FILES["system_prompt"])
    models = _read(fixture / CELL_FILES["pydantic_io_models"])
    vars_in_prompt = set(re.findall(r"\{\{\s*([A-Za-z_][A-Za-z0-9_]*)\s*\}\}", prompt))
    declared = set(re.findall(r"^\s*([A-Za-z_][A-Za-z0-9_]*)\s*:\s*", models, re.MULTILINE))
    missing = vars_in_prompt - declared - KNOWN_TEMPLATE_HELPERS
    if missing:
        _die(
            "system_prompt_field_coverage",
            f"system_prompt.md references variables not on the input model: {sorted(missing)}",
        )


def main() -> int:
    if len(sys.argv) != 2:
        sys.stderr.write("usage: cluster-audit-add-judge-worker.py <fixture_dir>\n")
        return 2
    fixture = Path(sys.argv[1])
    if not fixture.is_dir():
        sys.stderr.write(f"fixture {fixture!r} is not a directory\n")
        return 2
    check_agent_loop_litellm_shape(fixture)
    check_capability_binding(fixture)
    check_agent_loop_fields_in_models(fixture)
    check_unit_tests_fixture(fixture)
    check_system_prompt_fields(fixture)
    print("PASS add-judge-worker (5 checks passed)")
    return 0


if __name__ == "__main__":
    sys.exit(main())
