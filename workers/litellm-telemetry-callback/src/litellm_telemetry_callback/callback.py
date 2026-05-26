"""POSTs LiteLLM call telemetry to pipeline-cli's /llm-call-telemetry endpoint."""

from __future__ import annotations

import os
from dataclasses import asdict, dataclass
from typing import Any

import httpx

try:
    from litellm.integrations.custom_logger import CustomLogger
except ImportError:  # pragma: no cover - litellm is optional at import time
    class CustomLogger:  # type: ignore[no-redef]
        """Minimal stub used when LiteLLM is not installed (tests, lint).

        The real `litellm.integrations.custom_logger.CustomLogger` provides
        the same surface. The stub keeps this module importable in
        environments that vendor the callback without pulling LiteLLM.
        """


PIPELINE_ENDPOINT_ENV = "PIPELINE_ENDPOINT"
PIPELINE_TOKEN_ENV = "PIPELINE_TOKEN"
TELEMETRY_PATH = "/llm-call-telemetry"
DEFAULT_TIMEOUT_S = 5.0


@dataclass
class TelemetryRecord:
    """One LiteLLM call's worth of telemetry, indexed by ddd_session_id."""

    ddd_session_id: str
    model: str
    provider: str
    capability_tag: str
    input_tokens: int
    output_tokens: int
    cost_usd: float
    latency_ms: int
    retry_count: int
    fallback_chain: list[str]


def _coerce_int(value: Any) -> int:
    try:
        return int(value)
    except (TypeError, ValueError):
        return 0


def _coerce_float(value: Any) -> float:
    try:
        return float(value)
    except (TypeError, ValueError):
        return 0.0


def _coerce_list_of_str(value: Any) -> list[str]:
    if value is None:
        return []
    if isinstance(value, str):
        return [value]
    try:
        return [str(item) for item in value]
    except TypeError:
        return []


def build_record(
    kwargs: dict[str, Any],
    response_obj: Any,
    start_time: Any,
    end_time: Any,
) -> TelemetryRecord:
    """Lift a LiteLLM logging-hook call into a TelemetryRecord."""
    metadata = kwargs.get("litellm_params", {}).get("metadata", {}) or {}
    if not metadata:
        metadata = kwargs.get("metadata", {}) or {}
    usage = _extract_usage(response_obj)
    return TelemetryRecord(
        ddd_session_id=str(metadata.get("ddd_session_id", "")),
        model=str(kwargs.get("model", "") or ""),
        provider=str(
            kwargs.get("litellm_params", {}).get("custom_llm_provider", "")
            or kwargs.get("custom_llm_provider", "")
            or ""
        ),
        capability_tag=str(metadata.get("capability_tag", "") or kwargs.get("model", "")),
        input_tokens=_coerce_int(usage.get("prompt_tokens")),
        output_tokens=_coerce_int(usage.get("completion_tokens")),
        cost_usd=_coerce_float(kwargs.get("response_cost") or 0.0),
        latency_ms=_latency_ms(start_time, end_time),
        retry_count=_coerce_int(kwargs.get("num_retries")),
        fallback_chain=_coerce_list_of_str(kwargs.get("fallbacks")),
    )


def _extract_usage(response_obj: Any) -> dict[str, Any]:
    usage = getattr(response_obj, "usage", None)
    if usage is None and isinstance(response_obj, dict):
        usage = response_obj.get("usage")
    if usage is None:
        return {}
    if hasattr(usage, "model_dump"):
        return usage.model_dump()
    if hasattr(usage, "dict"):
        return usage.dict()
    if isinstance(usage, dict):
        return usage
    return {}


def _latency_ms(start_time: Any, end_time: Any) -> int:
    try:
        delta = end_time - start_time
    except TypeError:
        return 0
    total = getattr(delta, "total_seconds", None)
    if callable(total):
        return int(total() * 1000)
    try:
        return int(float(delta) * 1000)
    except (TypeError, ValueError):
        return 0


class PipelineCliTelemetryCallback(CustomLogger):
    """LiteLLM custom callback that ships telemetry to pipeline-cli.

    LiteLLM discovers this class via the `pipeline-cli-telemetry` entry
    in `litellm_settings.callbacks` (see config/litellm.yaml). The proxy
    invokes `log_success_event` / `async_log_success_event` /
    `async_log_failure_event` once per LLM completion; each invocation
    is lifted to a TelemetryRecord, then POSTed to pipeline-cli's
    /llm-call-telemetry endpoint as JSON.
    """

    def __init__(
        self,
        endpoint: str | None = None,
        token: str | None = None,
        timeout_s: float = DEFAULT_TIMEOUT_S,
    ) -> None:
        self.endpoint = endpoint or os.environ.get(PIPELINE_ENDPOINT_ENV, "")
        self.token = token or os.environ.get(PIPELINE_TOKEN_ENV, "")
        self.timeout_s = timeout_s

    def _telemetry_url(self) -> str:
        if not self.endpoint:
            return ""
        return self.endpoint.rstrip("/") + TELEMETRY_PATH

    def _headers(self) -> dict[str, str]:
        headers = {"content-type": "application/json"}
        if self.token:
            headers["authorization"] = f"Bearer {self.token}"
        return headers

    def log_success_event(
        self,
        kwargs: dict[str, Any],
        response_obj: Any,
        start_time: Any,
        end_time: Any,
    ) -> None:
        """Sync hook — POSTs telemetry via a short-lived httpx client."""
        url = self._telemetry_url()
        if not url:
            return
        record = build_record(kwargs, response_obj, start_time, end_time)
        with httpx.Client(timeout=self.timeout_s) as client:
            client.post(url, json=asdict(record), headers=self._headers())

    async def async_log_success_event(
        self,
        kwargs: dict[str, Any],
        response_obj: Any,
        start_time: Any,
        end_time: Any,
    ) -> None:
        """Async hook — POSTs telemetry via an async httpx client."""
        url = self._telemetry_url()
        if not url:
            return
        record = build_record(kwargs, response_obj, start_time, end_time)
        async with httpx.AsyncClient(timeout=self.timeout_s) as client:
            await client.post(url, json=asdict(record), headers=self._headers())

    async def async_log_failure_event(
        self,
        kwargs: dict[str, Any],
        response_obj: Any,
        start_time: Any,
        end_time: Any,
    ) -> None:
        """Failure hook — emits a telemetry record so reconciliation sees the call."""
        await self.async_log_success_event(kwargs, response_obj, start_time, end_time)
