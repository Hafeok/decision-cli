"""Tests for the proactive Scaleway rate-limit throttle in agent/loop.py.

Locks in:
- Header extraction across LiteLLM's varied response shapes.
- Reset-duration parsing for `Nms`, `N.Ns`, and bare numbers.
- Throttle fires when remaining capacity drops below the threshold.
- Throttle is silent when capacity is comfortable.
- Missing / malformed headers degrade to no-op (never crash, never sleep).
- Sleep duration is capped so a malformed reset header can't hang the loop.

See docs/scaleway-rate-limits.md for the underlying contract.
"""

from __future__ import annotations

import time

import pytest

from code_writer.agent.loop import (
    _extract_rate_limit_headers,
    _maybe_throttle,
    _parse_int,
    _parse_reset_duration,
)


# ---------------------------------------------------------------------
# _parse_int.
# ---------------------------------------------------------------------

def test_parse_int_well_formed():
    assert _parse_int("123") == 123
    assert _parse_int("  42  ") == 42


def test_parse_int_safe_on_garbage():
    assert _parse_int(None) is None
    assert _parse_int("") is None
    assert _parse_int("abc") is None
    assert _parse_int("3.14") is None  # int() rejects floats


# ---------------------------------------------------------------------
# _parse_reset_duration.
# ---------------------------------------------------------------------

def test_parse_reset_duration_milliseconds():
    assert _parse_reset_duration("250ms") == pytest.approx(0.25)
    assert _parse_reset_duration("35ms") == pytest.approx(0.035)


def test_parse_reset_duration_seconds():
    assert _parse_reset_duration("1.5s") == pytest.approx(1.5)
    assert _parse_reset_duration("10s") == pytest.approx(10.0)


def test_parse_reset_duration_bare_number_assumed_ms():
    # Per Scaleway docs the observed format is "Nms" — bare numbers default
    # to milliseconds for safety (assuming worst-case unit interpretation).
    assert _parse_reset_duration("750") == pytest.approx(0.75)


def test_parse_reset_duration_garbage_returns_none():
    assert _parse_reset_duration(None) is None
    assert _parse_reset_duration("") is None
    assert _parse_reset_duration("garbage") is None


# ---------------------------------------------------------------------
# _extract_rate_limit_headers.
# ---------------------------------------------------------------------

def test_extract_headers_from_hidden_params_additional_headers():
    class Resp:
        _hidden_params = {
            "additional_headers": {
                "X-RateLimit-Limit-Tokens": "400000",
                "X-RateLimit-Remaining-Tokens": "300000",
            }
        }

    h = _extract_rate_limit_headers(Resp())
    assert h.get("x-ratelimit-limit-tokens") == "400000"
    assert h.get("x-ratelimit-remaining-tokens") == "300000"


def test_extract_headers_from_response_headers_attr():
    class Resp:
        response_headers = {"x-ratelimit-limit-requests": "600"}

    h = _extract_rate_limit_headers(Resp())
    assert h.get("x-ratelimit-limit-requests") == "600"


def test_extract_headers_returns_empty_when_absent():
    class Resp:
        pass

    assert _extract_rate_limit_headers(Resp()) == {}


def test_extract_headers_keys_are_lowercased():
    class Resp:
        headers = {"X-RateLimit-Reset-Tokens": "35ms"}

    h = _extract_rate_limit_headers(Resp())
    assert "x-ratelimit-reset-tokens" in h
    assert "X-RateLimit-Reset-Tokens" not in h


# ---------------------------------------------------------------------
# _maybe_throttle.
# ---------------------------------------------------------------------

def test_throttle_no_op_above_threshold(capsys):
    headers = {
        "x-ratelimit-limit-tokens": "400000",
        "x-ratelimit-remaining-tokens": "300000",  # 75% — well above 10%
        "x-ratelimit-reset-tokens": "100ms",
    }
    start = time.monotonic()
    _maybe_throttle(headers, threshold_pct=10)
    elapsed = time.monotonic() - start
    assert elapsed < 0.05, "should not sleep when remaining is comfortable"
    captured = capsys.readouterr()
    assert "agent throttle" not in captured.err


def test_throttle_fires_below_threshold(capsys):
    headers = {
        "x-ratelimit-limit-tokens": "400000",
        "x-ratelimit-remaining-tokens": "5000",  # 1.25% — below 10%
        "x-ratelimit-reset-tokens": "100ms",
    }
    start = time.monotonic()
    _maybe_throttle(headers, threshold_pct=10)
    elapsed = time.monotonic() - start
    # Should sleep ~0.1s (the reset window).
    assert 0.05 < elapsed < 1.0, f"expected ~0.1s sleep, got {elapsed:.3f}s"
    captured = capsys.readouterr()
    assert "agent throttle" in captured.err
    assert "tokens remaining 5000/400000" in captured.err


def test_throttle_takes_longest_dimension_sleep():
    """If both tokens and requests are below threshold, sleep for the
    LONGER reset so both windows clear."""
    headers = {
        "x-ratelimit-limit-tokens": "400000",
        "x-ratelimit-remaining-tokens": "5000",
        "x-ratelimit-reset-tokens": "100ms",  # short
        "x-ratelimit-limit-requests": "600",
        "x-ratelimit-remaining-requests": "5",
        "x-ratelimit-reset-requests": "500ms",  # longer — wins
    }
    start = time.monotonic()
    _maybe_throttle(headers, threshold_pct=10)
    elapsed = time.monotonic() - start
    # Should sleep ~0.5s (the longer of the two).
    assert 0.4 < elapsed < 1.0, f"expected ~0.5s sleep, got {elapsed:.3f}s"


def test_throttle_no_op_on_empty_headers():
    start = time.monotonic()
    _maybe_throttle({}, threshold_pct=10)
    assert time.monotonic() - start < 0.05


def test_throttle_no_op_on_malformed_headers():
    headers = {
        "x-ratelimit-limit-tokens": "garbage",
        "x-ratelimit-remaining-tokens": "also-garbage",
        "x-ratelimit-reset-tokens": "nope",
    }
    start = time.monotonic()
    _maybe_throttle(headers, threshold_pct=10)
    assert time.monotonic() - start < 0.05


def test_throttle_sleep_capped_at_75s():
    """A malformed (huge) reset value must not hang the loop indefinitely.
    The cap is documented at 75s — generous margin over the 60s per-minute
    window."""
    # Simulate a reset header claiming 9999 seconds; throttle must clamp.
    # Use a very small threshold-violation to ensure the throttle fires.
    headers = {
        "x-ratelimit-limit-tokens": "400000",
        "x-ratelimit-remaining-tokens": "0",
        "x-ratelimit-reset-tokens": "9999s",
    }
    # We don't want the test to actually wait 75s; just verify the cap
    # logic by directly inspecting the sleep argument via monkeypatch.
    sleeps: list[float] = []
    import code_writer.agent.loop as loop_mod

    original_sleep = loop_mod.time.sleep
    loop_mod.time.sleep = sleeps.append  # type: ignore[assignment]
    try:
        _maybe_throttle(headers, threshold_pct=10)
    finally:
        loop_mod.time.sleep = original_sleep  # type: ignore[assignment]

    assert sleeps, "throttle should have invoked sleep"
    assert sleeps[0] == 75.0, f"sleep must be capped at 75s, got {sleeps[0]}"
