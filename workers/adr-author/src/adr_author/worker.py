"""Dispatches the adr-author role via ModelRouter, returning an AdrProposal."""

from __future__ import annotations

import json
import os
import sys
from dataclasses import dataclass
from pathlib import Path
from typing import Any, Callable

from pydantic import ValidationError

from .body_check import check_body_completeness
from .bundle import AdrAuthorInput
from .output import AdrProposal, GapProposal
from .parsing import BARE_ACK_MIN_CHARS, check_bare_ack, extract_json_object
from .prompts import SYSTEM_PROMPT, build_retry_prompt, build_user_prompt
from .schema import build_proposal_response_schema

# The shared package lives in a sibling worker directory; make it
# importable without requiring an editable install.
_SHARED_SRC = Path(__file__).resolve().parents[3] / "_shared" / "src"
if _SHARED_SRC.exists() and str(_SHARED_SRC) not in sys.path:
    sys.path.insert(0, str(_SHARED_SRC))

# Stub mode remains testing infrastructure: a fixture-time switch
# that bypasses the router, not a policy lever.
STUB_ENV_VAR = "ADR_AUTHOR_STUB"


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

    proposal: AdrProposal
    telemetry: WorkerTelemetry


class WorkerError(Exception):
    """Raised when the worker cannot produce a valid AdrProposal."""


class BundleHashMismatch(WorkerError):
    """The worker's proposal echoed the wrong bundle_hash — protocol violation."""


# Shape: (system, user, model_id, max_tokens) -> (raw_json, tokens_in, tokens_out).
# Used by stubs and tests; the live router path supersedes this seam.
ModelCaller = Callable[[str, str, str, int], tuple[str, int, int]]


def _is_stub_mode() -> bool:
    return os.environ.get(STUB_ENV_VAR, "").strip() not in {"", "0", "false", "no"}


def run_author(
    bundle: AdrAuthorInput,
    *,
    caller: ModelCaller | None = None,
    router: Any | None = None,
) -> WorkerResult:
    """Single-shot dispatch with one re-prompt on validation failure.

    Resolution order:

    * ``caller`` (or stub mode) → direct ``ModelCaller`` invocation. Tests
      inject a caller to simulate model outputs without touching any SDK.
    * ``router`` → drives a :func:`ModelRouter.call` dispatch through the
      resolved endpoint.
    * Otherwise → a router is built from ``bundle.endpoint``. This is the
      production path; the dispatcher pins endpoint + model on the bundle.

    Behaviour (FT-130):

    1. Build system + user prompt from the bundle.
    2. Call the model once with the structured-output schema.
    3. Validate the response against AdrProposal AND echo the bundle_hash.
    4. For ``new`` proposals, verify the body contains every required H2
       section per ``adr_body_schema``.
    5. For ``acknowledgement`` proposals, verify ``reasoning`` is non-empty
       AND ≥ 40 chars (brief §4B's "non-bare" floor) BEFORE stdout.
    6. On failure, re-prompt ONCE with the schema violation message.
    7. If the retry also fails, fall back to ``kind: gap``.
    """
    import time

    if caller is None and _is_stub_mode():
        caller = _stub_caller

    started = time.monotonic()
    if caller is not None:
        proposal, tokens_in, tokens_out, attempts = _run_via_caller(bundle, caller)
    else:
        proposal, tokens_in, tokens_out, attempts = _run_via_router(bundle, router)

    # Bundle-hash echo check — FT-130 / ADR-073.
    if proposal.bundle_hash != bundle.bundle_hash:
        raise BundleHashMismatch(
            f"AdrProposal.bundle_hash ('{proposal.bundle_hash}') does not "
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
    bundle: AdrAuthorInput,
    caller: ModelCaller,
) -> tuple[AdrProposal, int, int, int]:
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
            proposal = _fallback_gap(bundle, validation_error)
    return proposal, tokens_in, tokens_out, attempts


def _run_via_router(
    bundle: AdrAuthorInput,
    router: Any | None,
) -> tuple[AdrProposal, int, int, int]:
    """Router-based dispatch (production path)."""
    if router is None:
        router = _build_router_for_bundle(bundle)

    from _shared.model_router import CallParams  # noqa: PLC0415

    params = CallParams(
        endpoint=bundle.endpoint,  # type: ignore[arg-type]
        model_identifier=bundle.model_id,
        max_tokens=bundle.max_tokens,
        temperature=0.0,
        reasoning_effort=bundle.parameters.get("reasoning_effort"),
        response_schema=build_proposal_response_schema(),
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
            proposal = _fallback_gap(bundle, validation_error)

    return proposal, tokens_in, tokens_out, attempts


def _fallback_gap(bundle: AdrAuthorInput, validation_error: str | None) -> AdrProposal:
    """Build a fallback Gap proposal after the retry budget is exhausted."""
    detail = validation_error or "model output failed validation after retry"
    return AdrProposal(
        kind="gap",
        bundle_hash=bundle.bundle_hash,
        gap=GapProposal(
            missing_information=["schema-conformant proposal"],
            reason=(
                "could not produce schema-conformant ADR proposal after retry: "
                + detail
            ),
        ),
    )


def _build_router_for_bundle(bundle: AdrAuthorInput) -> Any:
    """Construct a ``ModelRouter`` from the endpoint pinned on the bundle."""
    from _shared.model_router import build_router  # noqa: PLC0415

    try:
        return build_router(bundle.endpoint)
    except Exception as exc:  # noqa: BLE001
        raise WorkerError(
            f"could not build router for endpoint={bundle.endpoint!r}: {exc}"
        ) from exc


def _extract_router_text(response: Any) -> str:
    """Pull JSON content out of a :class:`ModelResponse`."""
    text = (response.text or "").strip()
    if text:
        return text
    tool_calls = list(getattr(response, "tool_calls", []) or [])
    if not tool_calls:
        return ""
    arguments = tool_calls[0].arguments or {}
    return json.dumps(arguments)


def _try_parse_proposal(
    raw: str, bundle: AdrAuthorInput
) -> tuple[AdrProposal | None, str | None]:
    """Strict JSON + Pydantic parse plus per-kind body / reasoning checks.

    The bare-ack defence runs BEFORE Pydantic validation so a whitespace
    -only ``reasoning`` (which Pydantic's raw-string ``min_length`` would
    accept) is caught explicitly — see :func:`check_bare_ack`. After
    Pydantic validation, the stripped length is rechecked belt-and-braces.
    """
    text = (raw or "").strip()
    if not text:
        return None, "model returned empty content"
    payload = extract_json_object(text)
    if payload is None:
        return None, "model output did not contain a JSON object"

    bare_ack_reason = check_bare_ack(payload)
    if bare_ack_reason is not None:
        return None, bare_ack_reason

    try:
        proposal = AdrProposal.model_validate(payload)
    except ValidationError as exc:
        return None, str(exc)

    return _validate_kind_invariants(proposal, bundle)


def _validate_kind_invariants(
    proposal: AdrProposal, bundle: AdrAuthorInput
) -> tuple[AdrProposal | None, str | None]:
    """Cross-field invariants per output.py / FT-130 (one branch per kind)."""
    if proposal.kind == "new":
        if proposal.new is None:
            return None, "kind='new' but `new` payload is missing"
        body_result = check_body_completeness(
            proposal.new.body,
            bundle.adr_body_schema.required_h2_sections,
        )
        if body_result.warnings or body_result.errors:
            diagnostics = body_result.errors + body_result.warnings
            return None, "body-schema validation failed: " + "; ".join(diagnostics)
    elif proposal.kind == "acknowledgement":
        if proposal.acknowledgement is None:
            return None, "kind='acknowledgement' but `acknowledgement` payload is missing"
        stripped = proposal.acknowledgement.reasoning.strip()
        if len(stripped) < BARE_ACK_MIN_CHARS:
            return None, (
                "bare-ack rejected at worker boundary: "
                f"acknowledgement.reasoning stripped to {len(stripped)} chars "
                f"(< {BARE_ACK_MIN_CHARS}) per FT-130 §4B"
            )
    elif proposal.kind == "gap":
        if proposal.gap is None:
            return None, "kind='gap' but `gap` payload is missing"
    return proposal, None


def _stub_caller(system: str, user: str, model_id: str, max_tokens: int) -> tuple[str, int, int]:
    """Deterministic stub used when ADR_AUTHOR_STUB=1."""
    bundle_hash = _find_bundle_hash_in_prompt(user)
    payload = {
        "kind": "gap",
        "bundle_hash": bundle_hash,
        "gap": {
            "missing_information": ["stub fixture"],
            "reason": (
                "stub mode (ADR_AUTHOR_STUB=1): worker returns a synthetic "
                "gap proposal so smoke-tests do not depend on a live model session."
            ),
        },
    }
    return json.dumps(payload), 0, 0


def _find_bundle_hash_in_prompt(user: str) -> str:
    """Heuristic — extract the bundle_hash line from the rendered prompt."""
    marker = "bundle_hash (echo this verbatim in your proposal):"
    for line in user.splitlines():
        if marker in line:
            return line.split(marker, 1)[1].strip().rstrip("*").strip()
    return ""
