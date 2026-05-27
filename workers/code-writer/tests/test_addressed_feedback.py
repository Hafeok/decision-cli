"""FT-108 follow-up: cover the addressed-feedback citation extractor.

The agent's final-summary text must contain a marker-delimited JSON
block whose `iris` field lists the addressed feedback IRIs. The Rust
accept path's WorkerIgnoredFeedback guard rejects dispatches whose
extracted list is empty when the bundle carried feedback — so the
extractor's correctness is load-bearing.
"""

from __future__ import annotations

from code_writer._subprocess_runner import _extract_addressed_feedback


def test_extracts_iris_from_well_formed_block() -> None:
    summary = (
        "I implemented the fix. Files were written under crates/...\n"
        "\n"
        "<<DEC_ADDRESSED_FEEDBACK>>\n"
        '{"iris": ["urn:dec:feedback:abc", "urn:dec:feedback:def"]}\n'
        "<<END_DEC_ADDRESSED_FEEDBACK>>\n"
    )
    iris = _extract_addressed_feedback(summary)
    assert iris == ["urn:dec:feedback:abc", "urn:dec:feedback:def"]


def test_returns_last_block_when_multiple_present() -> None:
    summary = (
        "<<DEC_ADDRESSED_FEEDBACK>>{\"iris\": [\"urn:dec:feedback:old\"]}<<END_DEC_ADDRESSED_FEEDBACK>>\n"
        "Reconsidered. Final answer:\n"
        "<<DEC_ADDRESSED_FEEDBACK>>{\"iris\": [\"urn:dec:feedback:new\"]}<<END_DEC_ADDRESSED_FEEDBACK>>\n"
    )
    iris = _extract_addressed_feedback(summary)
    assert iris == ["urn:dec:feedback:new"]


def test_empty_when_no_marker() -> None:
    assert _extract_addressed_feedback("just a summary, no markers") == []


def test_empty_when_malformed_json() -> None:
    summary = (
        "<<DEC_ADDRESSED_FEEDBACK>>\n"
        "not valid json\n"
        "<<END_DEC_ADDRESSED_FEEDBACK>>\n"
    )
    assert _extract_addressed_feedback(summary) == []


def test_empty_when_iris_field_missing() -> None:
    summary = (
        "<<DEC_ADDRESSED_FEEDBACK>>\n"
        '{"foo": ["bar"]}\n'
        "<<END_DEC_ADDRESSED_FEEDBACK>>\n"
    )
    assert _extract_addressed_feedback(summary) == []


def test_empty_when_input_empty() -> None:
    assert _extract_addressed_feedback("") == []
    assert _extract_addressed_feedback(None) == []  # type: ignore[arg-type]


def test_filters_non_string_iri_entries() -> None:
    summary = (
        "<<DEC_ADDRESSED_FEEDBACK>>\n"
        '{"iris": ["urn:dec:feedback:ok", 42, null, "urn:dec:feedback:also-ok"]}\n'
        "<<END_DEC_ADDRESSED_FEEDBACK>>\n"
    )
    assert _extract_addressed_feedback(summary) == [
        "urn:dec:feedback:ok",
        "urn:dec:feedback:also-ok",
    ]
