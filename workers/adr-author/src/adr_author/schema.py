"""Builds the AdrProposal JSON schema for structured-output constraint."""

from __future__ import annotations


def _preflight_gap_schema() -> dict:
    return {
        "type": "object",
        "required": ["kind", "severity"],
        "properties": {
            "kind": {
                "type": "string",
                "enum": ["unacknowledged-adr", "uncovered-domain"],
            },
            "adr_id": {"type": ["string", "null"]},
            "domain": {"type": ["string", "null"]},
            "severity": {"type": "string", "enum": ["warning", "error"]},
            "message": {"type": "string"},
        },
    }


def build_proposal_response_schema() -> dict:
    """Return the JSON schema that constrains the model's structured output.

    Mirrors the Pydantic ``AdrProposal`` shape from output.py. The model
    is forced to respond with JSON matching this schema (Anthropic's
    structured-output mode or Scaleway's forced-tool-use equivalent).
    """
    return {
        "type": "object",
        "required": ["kind", "bundle_hash"],
        "properties": {
            "kind": {
                "type": "string",
                "enum": ["new", "acknowledgement", "gap"],
                "description": "Proposal kind discriminator.",
            },
            "bundle_hash": {
                "type": "string",
                "minLength": 8,
                "description": "Echo of input bundle_hash for verification.",
            },
            "new": {
                "type": "object",
                "required": [
                    "title",
                    "body",
                    "scope",
                    "addresses_gap",
                    "rationale",
                ],
                "properties": {
                    "title": {
                        "type": "string",
                        "minLength": 1,
                        "description": "Title for the proposed ADR.",
                    },
                    "body": {
                        "type": "string",
                        "minLength": 1,
                        "description": "Full markdown body, H2-conforming per adr_body_schema.",
                    },
                    "scope": {
                        "type": "string",
                        "enum": [
                            "cross-cutting",
                            "platform",
                            "domain",
                            "feature-specific",
                        ],
                    },
                    "proposed_domains": {
                        "type": "array",
                        "items": {"type": "string"},
                    },
                    "addresses_gap": _preflight_gap_schema(),
                    "rationale": {"type": "string", "minLength": 20},
                },
                "additionalProperties": False,
            },
            "acknowledgement": {
                "type": "object",
                "required": [
                    "acknowledges",
                    "target_feature",
                    "reasoning",
                    "rationale",
                ],
                "properties": {
                    "acknowledges": {
                        "type": "string",
                        "minLength": 1,
                        "description": "ADR-NNN id or domain name.",
                    },
                    "target_feature": {
                        "type": "string",
                        "minLength": 1,
                        "description": "FT-NNN this acknowledgement targets.",
                    },
                    "reasoning": {
                        "type": "string",
                        "minLength": 40,
                        "description": (
                            "Substantive reasoning (≥ 40 chars) — bare "
                            "acknowledgements are forbidden per FT-130 §4B."
                        ),
                    },
                    "rationale": {"type": "string", "minLength": 20},
                },
                "additionalProperties": False,
            },
            "gap": {
                "type": "object",
                "required": ["missing_information", "reason"],
                "properties": {
                    "missing_information": {
                        "type": "array",
                        "minItems": 1,
                        "items": {"type": "string"},
                        "description": "Concrete items the brief should clarify.",
                    },
                    "reason": {
                        "type": "string",
                        "minLength": 1,
                        "description": "Why the worker cannot author the ADR.",
                    },
                },
                "additionalProperties": False,
            },
        },
        "additionalProperties": False,
    }
