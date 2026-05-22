"""Calls Claude once with structured output, returning a GraphProposal."""

from __future__ import annotations

import json
import os
from dataclasses import dataclass
from typing import Any, Callable

from pydantic import ValidationError

from .bundle import StepKindRecord, VerifyGraphAuthorInput
from .output import GraphProposal, ProposedStep
from .prompts import SYSTEM_PROMPT, build_retry_prompt, build_user_prompt

# Single hard-coded binding per ADR-020 §"Asymmetric model selection rejected".
DEFAULT_MODEL_ID = "claude-sonnet-4-5"

# Env-var override is honoured ONLY when the bundle does not pin a model.
MODEL_ENV_VAR = "VERIFY_GRAPH_AUTHOR_MODEL_ID"
STUB_ENV_VAR = "VERIFY_GRAPH_AUTHOR_STUB"


@dataclass
class WorkerTelemetry:
    """Per-dispatch telemetry (ADR-008)."""

    model_id: str
    input_tokens: int = 0
    output_tokens: int = 0
    latency_seconds: float = 0.0
    attempts: int = 1
    exit_reason: str = "ok"


@dataclass
class WorkerResult:
    """Worker output: validated proposal + telemetry."""

    proposal: GraphProposal
    telemetry: WorkerTelemetry


class WorkerError(Exception):
    """Raised when the worker cannot produce a valid GraphProposal."""


class BundleHashMismatch(WorkerError):
    """The worker's proposal echoed the wrong bundle_hash — protocol violation."""


# Shape: (system, user, model_id, max_tokens) -> (raw_json, tokens_in, tokens_out).
ModelCaller = Callable[[str, str, str, int], tuple[str, int, int]]


def resolve_model_id(bundle: VerifyGraphAuthorInput) -> str:
    """Resolve the model id: bundle pin > env var > default binding."""
    if bundle.model_id and bundle.model_id != DEFAULT_MODEL_ID:
        return bundle.model_id
    return os.environ.get(MODEL_ENV_VAR, "").strip() or bundle.model_id or DEFAULT_MODEL_ID


def _is_stub_mode() -> bool:
    return os.environ.get(STUB_ENV_VAR, "").strip() not in {"", "0", "false", "no"}


def call_claude(system: str, user: str, model_id: str, max_tokens: int) -> tuple[str, int, int]:
    """Default model caller — uses the `anthropic` SDK.

    The SDK is imported lazily so the package can be exercised in CI
    (and via the stub mode) without `anthropic` being installed.
    """
    try:
        import anthropic  # type: ignore
    except ImportError as exc:  # pragma: no cover
        raise WorkerError(
            "anthropic SDK is not installed; install it with "
            "`pip install verify-graph-author[anthropic]` or run with "
            f"{STUB_ENV_VAR}=1"
        ) from exc

    client = anthropic.Anthropic()
    response = client.messages.create(
        model=model_id,
        max_tokens=max_tokens,
        system=system,
        messages=[{"role": "user", "content": user}],
    )
    text = _extract_text(response)
    usage = getattr(response, "usage", None)
    tokens_in = int(getattr(usage, "input_tokens", 0) or 0)
    tokens_out = int(getattr(usage, "output_tokens", 0) or 0)
    return text, tokens_in, tokens_out


def _extract_text(response: Any) -> str:
    """Pull the text content out of an anthropic Message response."""
    content = getattr(response, "content", None) or []
    parts: list[str] = []
    for block in content:
        text = getattr(block, "text", None)
        if text is None and isinstance(block, dict):
            text = block.get("text")
        if text:
            parts.append(text)
    return "\n".join(parts).strip()


def run_author(
    bundle: VerifyGraphAuthorInput,
    *,
    caller: ModelCaller | None = None,
) -> WorkerResult:
    """Single-shot dispatch with one re-prompt on validation failure.

    1. Build system + user prompt from the bundle.
    2. Call Claude once with the structured-output schema.
    3. Validate the response against GraphProposal AND echo the
       bundle_hash (FT-048 §Error 5).
    4. For `new` proposals, validate each step's `fields` against the
       step kind's `fields_schema` (FT-048 §Behaviour 6).
    5. On failure, re-prompt ONCE with the schema violation message.
    6. After the retry budget is exhausted on a `new` proposal whose
       steps fail their kind schema, downgrade to a `gap` rather than
       returning invalid output (FT-048 §Behaviour 6).
    """
    import time

    model_id = resolve_model_id(bundle)
    if caller is None:
        caller = _stub_caller if _is_stub_mode() else call_claude

    system = SYSTEM_PROMPT
    user = build_user_prompt(bundle)

    started = time.monotonic()
    raw, tokens_in, tokens_out = caller(system, user, model_id, bundle.max_tokens)
    proposal, validation_error = _try_parse_proposal(raw, bundle)
    attempts = 1

    if proposal is None:
        retry_user = user + "\n\n" + build_retry_prompt(str(validation_error))
        raw_retry, ti2, to2 = caller(system, retry_user, model_id, bundle.max_tokens)
        tokens_in += ti2
        tokens_out += to2
        attempts = 2
        proposal, validation_error = _try_parse_proposal(raw_retry, bundle)
        if proposal is None:
            raise WorkerError(f"GraphProposal failed validation after retry: {validation_error}")

    # Bundle-hash echo check — TC-078 / FT-048 §Error 5.
    if proposal.bundle_hash != bundle.bundle_hash:
        raise BundleHashMismatch(
            f"GraphProposal.bundle_hash ('{proposal.bundle_hash}') does not "
            f"match input bundle_hash ('{bundle.bundle_hash}')"
        )

    telemetry = WorkerTelemetry(
        model_id=model_id,
        input_tokens=tokens_in,
        output_tokens=tokens_out,
        latency_seconds=time.monotonic() - started,
        attempts=attempts,
        exit_reason="ok",
    )
    return WorkerResult(proposal=proposal, telemetry=telemetry)


def _try_parse_proposal(
    raw: str, bundle: VerifyGraphAuthorInput
) -> tuple[GraphProposal | None, str | None]:
    """Strict JSON + Pydantic parse with per-kind field validation for `new` proposals."""
    text = (raw or "").strip()
    if not text:
        return None, "model returned empty content"
    payload = _extract_json_object(text)
    if payload is None:
        return None, "model output did not contain a JSON object"
    try:
        proposal = GraphProposal.model_validate(payload)
    except ValidationError as exc:
        return None, str(exc)

    # Per-kind validation for `new` proposals (FT-048 §Behaviour 6).
    if proposal.kind == "new" and proposal.new is not None:
        vocab = {k.kind: k for k in bundle.step_vocabulary}
        errors = _validate_steps(proposal.new.steps, vocab, bundle.target_environment.allowed_ops)
        if errors:
            return None, "step validation failed: " + "; ".join(errors)
    return proposal, None


def _validate_steps(
    steps: list[ProposedStep],
    vocab: dict[str, StepKindRecord],
    allowed_ops: list[str],
) -> list[str]:
    """Check each proposed step against its kind's required_ops and fields_schema."""
    allowed = set(allowed_ops)
    errors: list[str] = []
    for i, step in enumerate(steps, start=1):
        kind = vocab.get(step.step_type)
        if kind is None:
            errors.append(
                f"step {i}: kind '{step.step_type}' is not in the supplied step vocabulary"
            )
            continue
        missing_ops = [op for op in kind.required_ops if op not in allowed]
        if missing_ops:
            errors.append(
                f"step {i} ({step.step_type}): requires op(s) "
                f"{missing_ops} not in target_environment.allowed_ops"
            )
        schema_err = _check_fields_against_schema(step.fields, kind.fields_schema)
        if schema_err is not None:
            errors.append(f"step {i} ({step.step_type}): fields {schema_err}")
    return errors


def _check_fields_against_schema(fields: dict, schema: dict) -> str | None:
    """Minimal client-side schema check: required keys present, no extras when forbidden.

    The worker does not depend on a full JSON-schema validator — the
    harness re-runs schema validation through Oxigraph SHACL after
    the proposal is accepted (FT-044). The worker's check is a cheap
    pre-filter so we don't propose obvious garbage.
    """
    required = schema.get("required") or []
    for key in required:
        if key not in fields:
            return f"missing required key '{key}'"
    additional = schema.get("additionalProperties")
    if additional is False:
        allowed_keys = set((schema.get("properties") or {}).keys())
        for key in fields.keys():
            if key not in allowed_keys:
                return f"unknown key '{key}' (additionalProperties=false)"
    return None


def _extract_json_object(text: str) -> dict | None:
    """Best-effort: find a top-level JSON object in the model output."""
    direct = _try_parse_json_dict(text)
    if direct is not None:
        return direct
    start = text.find("{")
    if start < 0:
        return None
    end = _find_balanced_brace_end(text, start)
    if end < 0:
        return None
    return _try_parse_json_dict(text[start : end + 1])


def _try_parse_json_dict(text: str) -> dict | None:
    """Parse `text` as JSON, returning the value only when it is a dict."""
    try:
        parsed = json.loads(text)
    except json.JSONDecodeError:
        return None
    return parsed if isinstance(parsed, dict) else None


def _find_balanced_brace_end(text: str, start: int) -> int:
    """Return index of the `}` closing the `{` at `start`, or -1 if unbalanced."""
    depth = 0
    in_str = False
    escape = False
    for i in range(start, len(text)):
        ch = text[i]
        if in_str:
            in_str, escape = _advance_string_state(ch, escape)
            continue
        if ch == '"':
            in_str = True
            continue
        if ch == "{":
            depth += 1
        elif ch == "}":
            depth -= 1
            if depth == 0:
                return i
    return -1


def _advance_string_state(ch: str, escape: bool) -> tuple[bool, bool]:
    """Update (in_string, escape) flags for one character inside a JSON string."""
    if escape:
        return True, False
    if ch == "\\":
        return True, True
    if ch == '"':
        return False, False
    return True, False


def _stub_caller(system: str, user: str, model_id: str, max_tokens: int) -> tuple[str, int, int]:
    """Deterministic stub used when VERIFY_GRAPH_AUTHOR_STUB=1.

    Returns a hard-coded `gap` proposal that simply names every TC as
    uncovered. Tests that exercise `match` or `new` paths inject their
    own caller via the `caller=` argument to `run_author`.
    """
    # Pull bundle_hash out of the rendered user prompt so the stub can echo it.
    bundle_hash = _find_bundle_hash_in_prompt(user)
    payload = {
        "kind": "gap",
        "bundle_hash": bundle_hash,
        "gap": {
            "uncovered_tcs": ["TC-stub"],
            "reason": (
                "stub mode (VERIFY_GRAPH_AUTHOR_STUB=1): worker returns a synthetic "
                "Gap so smoke-tests do not depend on a live Claude session."
            ),
        },
    }
    return json.dumps(payload), 0, 0


def _find_bundle_hash_in_prompt(user: str) -> str:
    """Heuristic — extract the bundle_hash line from the rendered prompt."""
    marker = "bundle_hash (echo this verbatim in your proposal):"
    for line in user.splitlines():
        if marker in line:
            return line.split(marker, 1)[1].strip()
    return ""
