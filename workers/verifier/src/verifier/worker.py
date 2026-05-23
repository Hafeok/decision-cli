"""Calls a routed model once with structured output, returning a VerificationVerdict."""

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

# Single hard-coded binding per ADR-020 §"Asymmetric model selection rejected".
DEFAULT_MODEL_ID = "claude-sonnet-4-5"

# Env-var override is honoured ONLY when the bundle does not pin a model.
# A bundle that pins a model wins (it is recorded in PROV-O for the
# session); the env var is the operator-side default for fixture / CI
# runs where the bundle does not specify one.
MODEL_ENV_VAR = "VERIFIER_MODEL_ID"
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


# A callable with shape `(system, user, model_id, max_tokens) -> (raw_json_string, tokens_in, tokens_out)`.
ModelCaller = Callable[[str, str, str, int], tuple[str, int, int]]


def resolve_model_id(bundle: VerifierInput) -> str:
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
    except ImportError as exc:  # pragma: no cover - exercised in environments without the SDK
        raise VerifierError(
            "anthropic SDK is not installed; install it with "
            "`pip install verifier[anthropic]` or run with VERIFIER_STUB=1"
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
        # `anthropic.types.TextBlock` exposes `.text`. Dict shape is the
        # fallback for test doubles.
        text = getattr(block, "text", None)
        if text is None and isinstance(block, dict):
            text = block.get("text")
        if text:
            parts.append(text)
    return "\n".join(parts).strip()


def run_verifier(
    bundle: VerifierInput,
    *,
    caller: ModelCaller | None = None,
) -> WorkerResult:
    """Single-shot dispatch with one re-prompt on validation failure.

    1. Build system + user prompt from the bundle.
    2. Call Claude once with the structured-output schema.
    3. Validate the response against VerificationVerdict.
    4. On failure, re-prompt ONCE with the schema violation message.
    5. Exit with `VerifierError` if the second response also fails.
    """
    import time

    model_id = resolve_model_id(bundle)
    if caller is None:
        caller = _stub_caller if _is_stub_mode() else call_claude

    system = SYSTEM_PROMPT
    user = build_user_prompt(bundle)

    started = time.monotonic()
    raw, tokens_in, tokens_out = caller(system, user, model_id, bundle.max_tokens)
    verdict, validation_error = _try_parse_verdict(raw)
    attempts = 1

    if verdict is None:
        retry_user = user + "\n\n" + build_retry_prompt(str(validation_error))
        raw_retry, ti2, to2 = caller(system, retry_user, model_id, bundle.max_tokens)
        tokens_in += ti2
        tokens_out += to2
        attempts = 2
        verdict, validation_error = _try_parse_verdict(raw_retry)
        if verdict is None:
            raise VerifierError(
                f"VerificationVerdict failed validation after retry: {validation_error}"
            )

    telemetry = WorkerTelemetry(
        model_id=model_id,
        input_tokens=tokens_in,
        output_tokens=tokens_out,
        latency_seconds=time.monotonic() - started,
        attempts=attempts,
        exit_reason="ok",
    )
    return WorkerResult(verdict=verdict, telemetry=telemetry)


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
# Router-based dispatch (FT-060)
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
    endpoint: str = "scaleway",
    reasoning_effort: str | None = None,
    exposes_reasoning_trace: bool = False,
) -> WorkerResult:
    """Dispatch the verifier through a :class:`ModelRouter`.

    This is the FT-060 entry point: instead of calling ``call_claude``
    directly, the worker constructs a ``CallParams`` envelope from the
    bundle and lets the router pick the endpoint client. The router
    returns a normalised :class:`ModelResponse` regardless of endpoint
    so the parse / retry logic is endpoint-agnostic.
    """
    import time

    params = build_call_params(
        bundle,
        endpoint=endpoint,
        reasoning_effort=reasoning_effort,
        exposes_reasoning_trace=exposes_reasoning_trace,
    )
    started = time.monotonic()
    system = SYSTEM_PROMPT
    user = build_user_prompt(bundle)
    response = router.call(system, user, params)
    raw = _extract_router_text(response)
    verdict, validation_error = _try_parse_verdict(raw)
    attempts = 1
    tokens_in = int(response.tokens_in)
    tokens_out = int(response.tokens_out)

    if verdict is None:
        retry_user = user + "\n\n" + build_retry_prompt(str(validation_error))
        retry_response = router.call(system, retry_user, params)
        tokens_in += int(retry_response.tokens_in)
        tokens_out += int(retry_response.tokens_out)
        attempts = 2
        retry_raw = _extract_router_text(retry_response)
        verdict, validation_error = _try_parse_verdict(retry_raw)
        if verdict is None:
            raise VerifierError(
                f"VerificationVerdict failed validation after retry: {validation_error}"
            )

    telemetry = WorkerTelemetry(
        model_id=params.model_identifier,
        input_tokens=tokens_in,
        output_tokens=tokens_out,
        latency_seconds=time.monotonic() - started,
        attempts=attempts,
        exit_reason="ok",
    )
    return WorkerResult(verdict=verdict, telemetry=telemetry)


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
