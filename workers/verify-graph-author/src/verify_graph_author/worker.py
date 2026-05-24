"""Dispatches the verify-graph-author role via ModelRouter, returning a GraphProposal."""

from __future__ import annotations

import json
import os
import sys
from dataclasses import dataclass
from pathlib import Path
from typing import Any, Callable

from pydantic import ValidationError

from .bundle import StepKindRecord, VerifyGraphAuthorInput
from .output import GraphProposal, ProposedStep
from .prompts import SYSTEM_PROMPT, build_retry_prompt, build_user_prompt
from .schema import build_proposal_response_schema

# The shared package lives in a sibling worker directory; make it
# importable without requiring an editable install.
_SHARED_SRC = Path(__file__).resolve().parents[3] / "_shared" / "src"
if _SHARED_SRC.exists() and str(_SHARED_SRC) not in sys.path:
    sys.path.insert(0, str(_SHARED_SRC))

# Stub mode remains testing infrastructure (FT-064 §invariants): it is not
# a policy lever, it is a fixture-time switch that bypasses the router.
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
# Used by the stub and by tests; the live router path supersedes this seam.
ModelCaller = Callable[[str, str, str, int], tuple[str, int, int]]


def _is_stub_mode() -> bool:
    return os.environ.get(STUB_ENV_VAR, "").strip() not in {"", "0", "false", "no"}


def run_author(
    bundle: VerifyGraphAuthorInput,
    *,
    caller: ModelCaller | None = None,
    router: Any | None = None,
) -> WorkerResult:
    """Single-shot dispatch with one re-prompt on validation failure.

    Resolution order matches the verifier (FT-064):

    * ``caller`` (or stub mode) → direct ``ModelCaller`` invocation. Tests
      inject a caller to simulate model outputs without touching any SDK.
    * ``router`` → drives a :func:`ModelRouter.call` dispatch through the
      resolved endpoint (FT-060).
    * Otherwise → a router is built from ``bundle.endpoint``. This is the
      production path; the dispatcher pins endpoint + model on the bundle
      per FT-061.

    Behaviour:

    1. Build system + user prompt from the bundle.
    2. Call the model once with the structured-output schema.
    3. Validate the response against GraphProposal AND echo the
       bundle_hash (FT-048 §Error 5).
    4. For `new` proposals, validate each step's `fields` against the
       step kind's `fields_schema` (FT-048 §Behaviour 6).
    5. On failure, re-prompt ONCE with the schema violation message.
    """
    import time

    if caller is None and _is_stub_mode():
        caller = _stub_caller

    started = time.monotonic()
    if caller is not None:
        proposal, tokens_in, tokens_out, attempts = _run_via_caller(bundle, caller)
    else:
        proposal, tokens_in, tokens_out, attempts = _run_via_router(bundle, router)

    # Bundle-hash echo check — TC-078 / FT-048 §Error 5.
    if proposal.bundle_hash != bundle.bundle_hash:
        raise BundleHashMismatch(
            f"GraphProposal.bundle_hash ('{proposal.bundle_hash}') does not "
            f"match input bundle_hash ('{bundle.bundle_hash}')"
        )

    telemetry = WorkerTelemetry(
        model_id=bundle.model_id,
        input_tokens=tokens_in,
        output_tokens=tokens_out,
        latency_seconds=time.monotonic() - started,
        attempts=attempts,
        exit_reason="ok",
    )
    return WorkerResult(proposal=proposal, telemetry=telemetry)


def _run_via_caller(
    bundle: VerifyGraphAuthorInput,
    caller: ModelCaller,
) -> tuple[GraphProposal, int, int, int]:
    """Direct ``ModelCaller`` path (stub / test seam)."""
    system = SYSTEM_PROMPT
    user = build_user_prompt(bundle)
    raw, tokens_in, tokens_out = caller(system, user, bundle.model_id, bundle.max_tokens)
    proposal, validation_error = _try_parse_proposal(raw, bundle)
    attempts = 1

    if proposal is None:
        retry_user = user + "\n\n" + build_retry_prompt(str(validation_error))
        raw_retry, ti2, to2 = caller(system, retry_user, bundle.model_id, bundle.max_tokens)
        tokens_in += ti2
        tokens_out += to2
        attempts = 2
        proposal, validation_error = _try_parse_proposal(raw_retry, bundle)
        if proposal is None:
            raise WorkerError(
                f"GraphProposal failed validation after retry: {validation_error}"
            )
    return proposal, tokens_in, tokens_out, attempts


def _run_via_router(
    bundle: VerifyGraphAuthorInput,
    router: Any | None,
) -> tuple[GraphProposal, int, int, int]:
    """Router-based dispatch (FT-060 production path)."""
    if router is None:
        router = _build_router_for_bundle(bundle)

    from _shared.model_router import CallParams  # noqa: PLC0415

    params = CallParams(
        endpoint=bundle.endpoint,  # type: ignore[arg-type]
        model_identifier=bundle.model_id,
        max_tokens=bundle.max_tokens,
        temperature=0.0,
        reasoning_effort=bundle.parameters.get("reasoning_effort"),
        response_schema=build_proposal_response_schema(bundle.step_vocabulary),
        exposes_reasoning_trace=bool(
            bundle.parameters.get("exposes_reasoning_trace", False)
        ),
    )

    system = SYSTEM_PROMPT
    user = build_user_prompt(bundle)
    try:
        response = router.call(system, user, params)
    except Exception as exc:  # noqa: BLE001 — surface as WorkerError
        raise WorkerError(f"router call failed: {exc}") from exc
    raw = _extract_router_text(response)
    proposal, validation_error = _try_parse_proposal(raw, bundle)
    attempts = 1
    tokens_in = int(response.tokens_in)
    tokens_out = int(response.tokens_out)

    if proposal is None:
        retry_user = user + "\n\n" + build_retry_prompt(str(validation_error))
        try:
            retry_response = router.call(system, retry_user, params)
        except Exception as exc:  # noqa: BLE001
            raise WorkerError(f"router call failed on retry: {exc}") from exc
        tokens_in += int(retry_response.tokens_in)
        tokens_out += int(retry_response.tokens_out)
        attempts = 2
        retry_raw = _extract_router_text(retry_response)
        proposal, validation_error = _try_parse_proposal(retry_raw, bundle)
        if proposal is None:
            raise WorkerError(
                f"GraphProposal failed validation after retry: {validation_error}"
            )

    return proposal, tokens_in, tokens_out, attempts


def _build_router_for_bundle(bundle: VerifyGraphAuthorInput) -> Any:
    """Construct a ``ModelRouter`` from the endpoint pinned on the bundle."""
    from _shared.model_router import build_router  # noqa: PLC0415

    try:
        return build_router(bundle.endpoint)
    except Exception as exc:  # noqa: BLE001
        raise WorkerError(
            f"could not build router for endpoint={bundle.endpoint!r}: {exc}"
        ) from exc


def _extract_router_text(response: Any) -> str:
    """Pull JSON content out of a :class:`ModelResponse`.

    Structured-output on Anthropic surfaces the proposal via a forced
    ``submit_verdict`` tool-use block (the router uses one ``submit_*``
    tool name for all structured outputs); on Scaleway it lives in
    ``message.content``. The parser is JSON-tolerant either way.
    """
    text = (response.text or "").strip()
    if text:
        return text
    tool_calls = list(getattr(response, "tool_calls", []) or [])
    if not tool_calls:
        return ""
    arguments = tool_calls[0].arguments or {}
    return json.dumps(arguments)


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
                "Gap so smoke-tests do not depend on a live model session."
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
