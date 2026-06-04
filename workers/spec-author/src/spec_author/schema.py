"""Builds the SpecProposal JSON schema for structured-output constraint."""

from __future__ import annotations


def build_proposal_response_schema() -> dict:
    """Return the JSON schema that constrains the model's structured output.

    Mirrors the Pydantic SpecProposal shape from output.py. The model is
    forced to respond with JSON matching this schema (Anthropic's
    structured-output mode or Scaleway's forced-tool-use equivalent).
    """
    return {
        "type": "object",
        "required": ["kind", "bundle_hash"],
        "properties": {
            "kind": {
                "type": "string",
                "enum": ["new", "gap"],
                "description": "Proposal kind discriminator.",
            },
            "bundle_hash": {
                "type": "string",
                "minLength": 8,
                "description": "Echo of input bundle_hash for verification.",
            },
            "new": {
                "type": "object",
                "required": ["title", "body", "rationale"],
                "properties": {
                    "title": {
                        "type": "string",
                        "minLength": 1,
                        "description": "Title for the proposed feature_spec.",
                    },
                    "body": {
                        "type": "string",
                        "minLength": 1,
                        "description": "Full markdown body, H2/H3-conforming per ADR-047.",
                    },
                    "proposed_depends_on": {
                        "type": "array",
                        "items": {"type": "string"},
                        "description": "FT-NNN identifiers this spec depends on.",
                    },
                    "proposed_adrs": {
                        "type": "array",
                        "items": {"type": "string"},
                        "description": "ADR-NNN identifiers the spec respects.",
                    },
                    "proposed_domains": {
                        "type": "array",
                        "items": {"type": "string"},
                        "description": "Domain names from domain_registry.",
                    },
                    "rationale": {
                        "type": "string",
                        "minLength": 20,
                        "description": "Why this body addresses the request.",
                    },
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
                        "description": "Why the worker cannot author the spec.",
                    },
                },
                "additionalProperties": False,
            },
        },
        "additionalProperties": False,
    }
