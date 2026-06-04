"""Structured-output response schema for adr-quality verdicts."""

from __future__ import annotations


def build_verdict_response_schema() -> dict:
    """Return the JSON schema for QualityVerdict structured output (ADR-074, FT-133).

    Constrains the model to emit a valid QualityVerdict with:
    - verdict in {'approved', 'rejected', 'amendment-required'}
    - rationale (string)
    - judges (string IRI)
    - against (array of string IRIs — for ADR quality, the preflight_gap IRI + feature_spec IRI)
    - violates (array, conditionally required)
    - amendment_guidance (string, conditionally required)
    - bundle_hash (string)

    The schema does NOT enforce the ADR-018 cross-field constraints
    (rationale >= 20 chars, violates required for rejected/amendment-required,
    amendment_guidance required for amendment-required) — those are validated
    in the Pydantic model after parsing.
    """
    return {
        "type": "object",
        "properties": {
            "verdict": {
                "type": "string",
                "enum": ["approved", "rejected", "amendment-required"],
                "description": "Judgment: approved, rejected, or amendment-required.",
            },
            "rationale": {
                "type": "string",
                "description": "Substantive explanation for the verdict (>= 20 chars).",
            },
            "judges": {
                "type": "string",
                "description": "IRI of the AdrProposal artifact under judgment.",
            },
            "against": {
                "type": "array",
                "items": {"type": "string"},
                "minItems": 1,
                "description": (
                    "Source-of-truth IRIs (preflight_gap IRI + feature_spec IRI "
                    "for ADR quality)."
                ),
            },
            "violates": {
                "type": "array",
                "items": {"type": "string"},
                "description": "IRIs or identifiers of violated references (required for rejected/amendment-required).",
            },
            "amendment_guidance": {
                "type": "string",
                "description": "Concrete guidance for amendment-required verdicts.",
            },
            "bundle_hash": {
                "type": "string",
                "description": "Echoed bundle_hash from the input.",
            },
        },
        "required": ["verdict", "rationale", "judges", "against", "bundle_hash"],
        "additionalProperties": False,
    }
