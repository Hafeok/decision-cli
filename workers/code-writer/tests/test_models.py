"""Model invariants for the dispatch/response surface."""

from __future__ import annotations

import pytest
from pydantic import ValidationError

from code_writer.models import CodeChange, DispatchPayload, WorkerResponse


def test_dispatch_payload_requires_core_fields() -> None:
    with pytest.raises(ValidationError):
        DispatchPayload.model_validate({})


def test_dispatch_payload_round_trip() -> None:
    blob = {
        "dispatch_id": "urn:dec:dispatch:1",
        "session_id": "urn:dec:session:1",
        "feature_id": "FT-013",
        "bundle_markdown": "# bundle",
        "bundle_hash": "deadbeef" * 8,
        "workspace_path": "/tmp/ws",
        "model_id": "claude-sonnet-4-5",
        "endpoint": "anthropic",
    }
    payload = DispatchPayload.model_validate(blob)
    assert payload.feature_id == "FT-013"
    assert payload.max_turns == 8
    assert "Edit" in payload.allowed_tools
    assert payload.endpoint == "anthropic"


def test_code_change_links_session_for_provo() -> None:
    cc = CodeChange(
        iri="urn:dec:cc:1",
        feature_id="FT-013",
        session_id="urn:dec:session:1",
    )
    assert cc.session_id == "urn:dec:session:1"
    assert cc.files == []


def test_worker_response_error_shape() -> None:
    response = WorkerResponse.model_validate(
        {
            "dispatch_id": "urn:dec:dispatch:1",
            "session_id": "urn:dec:session:1",
            "status": "error",
            "error": {
                "category": "subprocess_failed",
                "message": "boom",
            },
        }
    )
    assert response.status == "error"
    assert response.error is not None
    assert response.error.category == "subprocess_failed"
