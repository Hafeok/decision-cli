"""Dispatches the verifier role through ModelRouter, returning a VerificationVerdict."""

from __future__ import annotations

import json
import os
import sys
from dataclasses import dataclass
from pathlib import Path
from typing import Any, Callable

from pydantic import ValidationError

from .bundle import VerifierInput
from .output import VerificationVerdict
from .prompts import SYSTEM_PROMPT, build_retry_prompt, build_user_prompt

# The shared package lives in a sibling worker directory; make it
# importable without requiring an editable install (mirrors the
# convention shared tests use).
_SHARED_SRC = Path(__file__).resolve().parents[3] / "_shared" / "src"
if _SHARED_SRC.exists() and str(_SHARED_SRC) not in sys.path:
    sys.path.insert(0, str(_SHARED_SRC))

# Stub mode remains testing infrastructure (FT-064 §Verifier worker changes):
# it is not a policy or model-binding lever, it is a fixture-time switch
# that bypasses the router entirely.
STUB_ENV_VAR = "VERIFIER_STUB"


@dataclass
class WorkerTelemetry:
    """Per-dispatch session telemetry (ADR-008)."""

    model_id: str
    input_tokens: int = 0
    output_tokens: int = 0
    latency_seconds: float = 0.0
    attempts: int = 1
    exit_reason: str = "ok"


@dataclass
class WorkerResult:
    """Verifier worker output: validated verdict + telemetry."""

    verdict: VerificationVerdict
    telemetry: WorkerTelemetry


class VerifierError(Exception):
    """Raised when the worker cannot produce a valid verdict."""


# A callable producing a raw JSON payload from a router-resolved dispatch.
# Shape: ``(system, user, model_id, max_tokens) -> (raw_json, tokens_in, tokens_out)``.
# Used by the stub and by tests that want to short-circuit the router seam.
ModelCaller = Callable[[str, str, str, int], tuple[str, int, int]]


def _is_stub_mode() -> bool:
    return os.environ.get(STUB_ENV_VAR, "").strip() not in {"", "0", "false", "no"}


def run_verifier(
    bundle: VerifierInput,
    *,
    caller: ModelCaller | None = None,
    router: Any | None = None,
) -> WorkerResult:
    """Single-shot dispatch with one re-prompt on validation failure.

    Resolution order:

    * ``caller`` (when supplied, or stub mode is active) → direct invocation
      bypassing the router. Tests inject a caller to simulate model outputs
      without touching any SDK.
    * ``router`` (when supplied) → drives a :func:`ModelRouter.call`
      dispatch through the resolved endpoint (FT-060). The router itself
      knows whether to talk to Anthropic or Scaleway based on the
      ``CallParams.endpoint`` field.
    * Otherwise → a router is constructed from ``bundle.endpoint`` via
      :func:`_shared.model_router.build_router`. This is the production
      path; the dispatcher pins the endpoint and model identifier on the
      bundle per FT-061.
    """
    import time

    if caller is None and _is_stub_mode():
        caller = _stub_caller

    started = time.monotonic()
    if caller is not None:
        verdict, tokens_in, tokens_out, attempts = _run_via_caller(
            bundle, caller, started
        )
    else:
        verdict, tokens_in, tokens_out, attempts = _run_via_router(
            bundle, router, started
        )

    telemetry = WorkerTelemetry(
        model_id=bundle.model_id,
        input_tokens=tokens_in,
        output_tokens=tokens_out,
        latency_seconds=time.monotonic() - started,
        attempts=attempts,
        exit_reason="ok",
    )
    return WorkerResult(verdict=verdict, telemetry=telemetry)


def _run_via_caller(
    bundle: VerifierInput,
    caller: ModelCaller,
    _started: float,
) -> tuple[VerificationVerdict, int, int, int]:
    """Direct ``ModelCaller`` path (stub / test seam)."""
    system = SYSTEM_PROMPT
    user = build_user_prompt(bundle)
    raw, tokens_in, tokens_out = caller(system, user, bundle.model_id, bundle.max_tokens)
    verdict, validation_error = _try_parse_verdict(raw)
    attempts = 1

    if verdict is None:
        retry_user = user + "\n\n" + build_retry_prompt(str(validation_error))
        raw_retry, ti2, to2 = caller(system, retry_user, bundle.model_id, bundle.max_tokens)
        tokens_in += ti2
        tokens_out += to2
        attempts = 2
        verdict, validation_error = _try_parse_verdict(raw_retry)
        if verdict is None:
            raise VerifierError(
                f"VerificationVerdict failed validation after retry: {validation_error}"
            )

    return verdict, tokens_in, tokens_out, attempts


def _run_via_router(
    bundle: VerifierInput,
    router: Any | None,
    _started: float,
) -> tuple[VerificationVerdict, int, int, int]:
    """Router-based dispatch (FT-060 production path)."""
    if router is None:
        router = _build_router_for_bundle(bundle)
    params = build_call_params(
        bundle,
        endpoint=bundle.endpoint,
        reasoning_effort=bundle.parameters.get("reasoning_effort"),
        exposes_reasoning_trace=bool(
            bundle.parameters.get("exposes_reasoning_trace", False)
        ),
    )
    system = SYSTEM_PROMPT
    user = build_user_prompt(bundle)
    try:
        response = router.call(system, user, params)
    except Exception as exc:  # noqa: BLE001 — surfaced as VerifierError below
        raise VerifierError(f"router call failed: {exc}") from exc
    raw = _extract_router_text(response)
    verdict, validation_error = _try_parse_verdict(raw)
    attempts = 1
    tokens_in = int(response.tokens_in)
    tokens_out = int(response.tokens_out)

    if verdict is None:
        retry_user = user + "\n\n" + build_retry_prompt(str(validation_error))
        try:
            retry_response = router.call(system, retry_user, params)
        except Exception as exc:  # noqa: BLE001 — surfaced as VerifierError below
            raise VerifierError(f"router call failed on retry: {exc}") from exc
        tokens_in += int(retry_response.tokens_in)
        tokens_out += int(retry_response.tokens_out)
        attempts = 2
        retry_raw = _extract_router_text(retry_response)
        verdict, validation_error = _try_parse_verdict(retry_raw)
        if verdict is None:
            raise VerifierError(
                f"VerificationVerdict failed validation after retry: {validation_error}"
            )

    return verdict, tokens_in, tokens_out, attempts


def _build_router_for_bundle(bundle: VerifierInput) -> Any:
    """Construct a ``ModelRouter`` from the endpoint pinned on the bundle."""
    from _shared.model_router import build_router  # noqa: PLC0415

    try:
        return build_router(bundle.endpoint)
    except Exception as exc:  # noqa: BLE001 — normalise to VerifierError
        raise VerifierError(
            f"could not build router for endpoint={bundle.endpoint!r}: {exc}"
        ) from exc


def _try_parse_verdict(raw: str) -> tuple[VerificationVerdict | None, str | None]:
    """Strict JSON + Pydantic parse. Returns (verdict, None) on success."""
    text = (raw or "").strip()
    if not text:
        return None, "model returned empty content"
    payload = _extract_json_object(text)
    if payload is None:
        return None, "model output did not contain a JSON object"
    try:
        return VerificationVerdict.model_validate(payload), None
    except ValidationError as exc:
        return None, str(exc)


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
    """Deterministic stub used when VERIFIER_STUB=1.

    Returns a hard-coded `approved` verdict; tests that need to exercise
    `rejected` / `amendment-required` paths inject their own caller via
    the `caller=` argument to `run_verifier`.
    """
    payload = {
        "verdict": "approved",
        "rationale": (
            "stub mode (VERIFIER_STUB=1): produced artifact assumed to satisfy "
            "the feature_spec and listed test criteria for harness smoke-tests."
        ),
        "violates": [],
    }
    return json.dumps(payload), 0, 0


# ---------------------------------------------------------------------------
# Router-based dispatch helpers (FT-060)
# ---------------------------------------------------------------------------


def build_call_params(
    bundle: VerifierInput,
    *,
    endpoint: str,
    reasoning_effort: str | None = None,
    exposes_reasoning_trace: bool = False,
) -> Any:
    """Construct a ``CallParams`` envelope from a verifier dispatch bundle.

    The verifier's structured-output schema (``VerificationVerdict``) is
    derived from the Pydantic model; ``model_identifier`` comes from the
    bundle (which the dispatcher pins per FT-061), never from a module
    constant. This is the seam TC-106 §8 asserts.
    """
    from _shared.model_router import CallParams  # noqa: PLC0415

    model_identifier = (bundle.model_id or "").strip()
    if not model_identifier:
        raise VerifierError(
            "bundle.model_id is empty; the dispatcher must pin the model "
            "identifier per FT-061 before router-based dispatch"
        )
    return CallParams(
        endpoint=endpoint,  # type: ignore[arg-type]
        model_identifier=model_identifier,
        max_tokens=bundle.max_tokens,
        temperature=0.0,
        reasoning_effort=reasoning_effort,  # type: ignore[arg-type]
        response_schema=VerificationVerdict.model_json_schema(),
        exposes_reasoning_trace=exposes_reasoning_trace,
    )


def run_verifier_via_router(
    bundle: VerifierInput,
    router: Any,
    *,
    endpoint: str | None = None,
    reasoning_effort: str | None = None,
    exposes_reasoning_trace: bool = False,
) -> WorkerResult:
    """Dispatch the verifier through a :class:`ModelRouter`.

    Kept as a stable public surface for FT-060 callers that wire their
    own router; internally it just delegates to :func:`run_verifier`
    with the router argument. The ``endpoint`` / ``reasoning_effort`` /
    ``exposes_reasoning_trace`` kwargs override the bundle's
    ``endpoint`` / ``parameters`` when supplied (test seam).
    """
    if endpoint is not None or reasoning_effort is not None or exposes_reasoning_trace:
        overridden = bundle.model_copy(
            update={
                "endpoint": endpoint if endpoint is not None else bundle.endpoint,
                "parameters": {
                    **bundle.parameters,
                    **(
                        {"reasoning_effort": reasoning_effort}
                        if reasoning_effort is not None
                        else {}
                    ),
                    **(
                        {"exposes_reasoning_trace": True}
                        if exposes_reasoning_trace
                        else {}
                    ),
                },
            }
        )
    else:
        overridden = bundle
    return run_verifier(overridden, router=router)


def _extract_router_text(response: Any) -> str:
    """Pull a JSON-shaped payload from a :class:`ModelResponse`.

    Structured-output dispatches on Anthropic surface the verdict as a
    forced ``submit_verdict`` ``tool_use``; on Scaleway the verdict is
    in ``message.content``. The verifier's parser is JSON-tolerant, so
    either flavour produces the same downstream behaviour.
    """
    text = (response.text or "").strip()
    if text:
        return text
    tool_calls = list(getattr(response, "tool_calls", []) or [])
    if not tool_calls:
        return ""
    arguments = tool_calls[0].arguments or {}
    return json.dumps(arguments)
