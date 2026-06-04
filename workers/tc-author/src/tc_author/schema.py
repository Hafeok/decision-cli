"""Builds the TcProposal JSON schema for structured-output constraint."""

from __future__ import annotations


def build_proposal_response_schema() -> dict:
    """Return the JSON schema that constrains the model's structured output.

    Mirrors the Pydantic TcProposal shape from output.py. The model is
    forced to respond with JSON matching this schema (Anthropic's
    structured-output mode or Scaleway's forced-tool-use equivalent).
    """
    return {
        "type": "object",
        "required": ["kind", "bundle_hash"],
        "properties": {
            "kind": {
                "type": "string",
                "enum": ["sufficient", "augment", "new"],
                "description": "Proposal kind discriminator.",
            },
            "bundle_hash": {
                "type": "string",
                "minLength": 8,
                "description": "Echo of input bundle_hash for verification.",
            },
            "sufficient": {
                "type": "object",
                "required": ["reasoning", "coverage_map"],
                "properties": {
                    "reasoning": {
                        "type": "string",
                        "minLength": 20,
                        "description": "Why existing TCs already meet target_count.",
                    },
                    "coverage_map": {
                        "type": "object",
                        "additionalProperties": {
                            "type": "array",
                            "items": {"type": "string"},
                        },
                        "description": "TC-id -> coverage_axes hit.",
                    },
                },
                "additionalProperties": False,
            },
            "augment": {
                "type": "object",
                "required": ["additions", "reasoning"],
                "properties": {
                    "retained_tcs": {
                        "type": "array",
                        "items": {"type": "string"},
                        "description": "TC-ids kept as-is.",
                    },
                    "additions": {
                        "type": "array",
                        "minItems": 1,
                        "items": {"$ref": "#/$defs/ProposedTc"},
                        "description": "New TCs to add.",
                    },
                    "reasoning": {
                        "type": "string",
                        "minLength": 20,
                        "description": "Why these additions bring coverage to target_count.",
                    },
                },
                "additionalProperties": False,
            },
            "new": {
                "type": "object",
                "required": ["tcs", "reasoning"],
                "properties": {
                    "replaced_tcs": {
                        "type": "array",
                        "items": {"type": "string"},
                        "description": "TC-ids the harness should supersede.",
                    },
                    "tcs": {
                        "type": "array",
                        "minItems": 1,
                        "items": {"$ref": "#/$defs/ProposedTc"},
                        "description": "Ordered, target_count entries.",
                    },
                    "reasoning": {
                        "type": "string",
                        "minLength": 20,
                        "description": "Why this TC set covers the feature.",
                    },
                },
                "additionalProperties": False,
            },
        },
        "additionalProperties": False,
        "$defs": {
            "ProposedTc": {
                "type": "object",
                "required": [
                    "title",
                    "type",
                    "body",
                    "runner",
                    "runner_args",
                    "observes",
                    "validates_features",
                ],
                "properties": {
                    "title": {
                        "type": "string",
                        "minLength": 1,
                        "description": "TC title.",
                    },
                    "type": {
                        "type": "string",
                        "enum": ["scenario", "exit-criteria", "invariant"],
                        "description": "TC type from the TC schema.",
                    },
                    "body": {
                        "type": "string",
                        "minLength": 1,
                        "description": "Full markdown body with H2 sections per TC schema.",
                    },
                    "runner": {
                        "type": "string",
                        "enum": ["bash", "cargo-test", "pytest", "custom"],
                        "description": "Runner kind from the runner vocabulary.",
                    },
                    "runner_args": {
                        "type": "string",
                        "minLength": 1,
                        "description": "Args for the runner — REQUIRED and non-empty.",
                    },
                    "runner_timeout": {
                        "type": "string",
                        "description": "Timeout for this TC (e.g. '60s').",
                    },
                    "observes": {
                        "type": "array",
                        "minItems": 1,
                        "items": {"type": "string"},
                        "description": "FT-072 observed surfaces — at least one required.",
                    },
                    "coverage_axis": {
                        "type": ["string", "null"],
                        "description": "Advisory tag from coverage_axes list.",
                    },
                    "validates_features": {
                        "type": "array",
                        "minItems": 1,
                        "items": {"type": "string"},
                        "description": "Feature IDs this TC validates.",
                    },
                    "validates_adrs": {
                        "type": "array",
                        "items": {"type": "string"},
                        "description": "Cross-cutting ADRs cited in the spec.",
                    },
                },
                "additionalProperties": False,
            }
        },
    }
