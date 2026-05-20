"""Calls Claude once with structured output, returning a VerificationVerdict."""

from __future__ import annotations

import json
import os
from dataclasses import dataclass
from typing import Any, Callable

from pydantic import ValidationError

from .bundle import VerifierInput
from .output import VerificationVerdict
from .prompts import SYSTEM_PROMPT, build_retry_prompt, build_user_prompt

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
    # Fast path: the whole string is JSON.
    try:
        obj = json.loads(text)
    except json.JSONDecodeError:
        obj = None
    if isinstance(obj, dict):
        return obj
    # Fallback: pull the first balanced `{...}` block out of the text.
    start = text.find("{")
    if start < 0:
        return None
    depth = 0
    in_str = False
    escape = False
    for i in range(start, len(text)):
        ch = text[i]
        if in_str:
            if escape:
                escape = False
            elif ch == "\\":
                escape = True
            elif ch == '"':
                in_str = False
            continue
        if ch == '"':
            in_str = True
            continue
        if ch == "{":
            depth += 1
        elif ch == "}":
            depth -= 1
            if depth == 0:
                snippet = text[start : i + 1]
                try:
                    parsed = json.loads(snippet)
                except json.JSONDecodeError:
                    return None
                return parsed if isinstance(parsed, dict) else None
    return None


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
