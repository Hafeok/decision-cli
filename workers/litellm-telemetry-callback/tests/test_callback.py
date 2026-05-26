"""Tests for the pipeline-cli-telemetry LiteLLM callback (FT-096)."""

from __future__ import annotations

from datetime import datetime, timedelta

import pytest

from litellm_telemetry_callback import PipelineCliTelemetryCallback, build_record


def _stub_response(prompt: int, completion: int) -> dict:
    return {"usage": {"prompt_tokens": prompt, "completion_tokens": completion}}


def test_build_record_extracts_session_metadata_and_usage() -> None:
    start = datetime(2026, 5, 26, 12, 0, 0)
    end = start + timedelta(milliseconds=250)
    kwargs = {
        "model": "frontier-reasoning",
        "litellm_params": {
            "custom_llm_provider": "anthropic",
            "metadata": {
                "ddd_session_id": "sess-abc",
                "capability_tag": "frontier-reasoning",
            },
        },
        "response_cost": 0.0125,
        "num_retries": 1,
        "fallbacks": ["fast-cheap"],
    }
    record = build_record(kwargs, _stub_response(120, 80), start, end)
    assert record.ddd_session_id == "sess-abc"
    assert record.model == "frontier-reasoning"
    assert record.provider == "anthropic"
    assert record.capability_tag == "frontier-reasoning"
    assert record.input_tokens == 120
    assert record.output_tokens == 80
    assert record.cost_usd == pytest.approx(0.0125)
    assert record.latency_ms == 250
    assert record.retry_count == 1
    assert record.fallback_chain == ["fast-cheap"]


def test_callback_posts_record_to_pipeline_endpoint(monkeypatch: pytest.MonkeyPatch) -> None:
    captured: dict = {}

    class _ClientStub:
        def __init__(self, *_, **__) -> None:
            pass

        def __enter__(self):
            return self

        def __exit__(self, *_exc) -> None:
            return None

        def post(self, url, json, headers):
            captured["url"] = url
            captured["json"] = json
            captured["headers"] = headers

    import litellm_telemetry_callback.callback as cb

    monkeypatch.setattr(cb.httpx, "Client", _ClientStub)
    callback = PipelineCliTelemetryCallback(
        endpoint="http://localhost:8080",
        token="tok-1",
    )
    start = datetime(2026, 5, 26, 12, 0, 0)
    end = start + timedelta(milliseconds=100)
    callback.log_success_event(
        kwargs={
            "model": "frontier-reasoning",
            "litellm_params": {
                "custom_llm_provider": "anthropic",
                "metadata": {"ddd_session_id": "sess-1"},
            },
            "response_cost": 0.001,
        },
        response_obj=_stub_response(10, 20),
        start_time=start,
        end_time=end,
    )
    assert captured["url"] == "http://localhost:8080/llm-call-telemetry"
    assert captured["json"]["ddd_session_id"] == "sess-1"
    assert captured["headers"]["authorization"] == "Bearer tok-1"


def test_callback_no_op_when_endpoint_unset(monkeypatch: pytest.MonkeyPatch) -> None:
    monkeypatch.delenv("PIPELINE_ENDPOINT", raising=False)
    callback = PipelineCliTelemetryCallback()
    callback.log_success_event(
        kwargs={"model": "x", "litellm_params": {}, "response_cost": 0.0},
        response_obj=_stub_response(0, 0),
        start_time=datetime(2026, 1, 1),
        end_time=datetime(2026, 1, 1),
    )
