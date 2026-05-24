"""Dynamic JSON Schema builder for the GraphProposal response shape.

Pydantic emits ``ProposedStep.fields`` as a freeform ``{"type": "object"}``
with no required keys and no per-kind discrimination, because
``ProposedStep.fields`` is statically typed as ``dict[str, Any]``. That
shape leaves the model free to emit ``fields: {}`` even when the bundle
explicitly declared keys as REQUIRED — Qwen3-Coder on Scaleway exploited
that latitude consistently, producing proposals that failed downstream
SHACL and the worker's own per-kind validator.

The fix: at call time, post-process the static schema to replace
``$defs/ProposedStep`` with a discriminated union (``oneOf``) where each
branch is a fully-typed step variant whose ``fields`` schema comes from
the bundle's ``step_vocabulary`` (the same ``fields_schema`` the Rust
``bundle.rs`` populates with kebab-case JSON Schemas). Each branch
requires ``step_type``, ``fields``, and ``provides_evidence_for`` —
removing the schema-level latitude that the model used to skip ``fields``.
"""

from __future__ import annotations

import copy
from typing import Any

from .bundle import StepKindRecord
from .output import GraphProposal


def build_proposal_response_schema(
    step_vocabulary: list[StepKindRecord],
) -> dict[str, Any]:
    """Return a JSON Schema where ProposedStep is a per-kind oneOf union.

    The base shape is ``GraphProposal.model_json_schema()`` with two
    transformations layered on top:

    1. ``$defs/ProposedStep`` is replaced with a ``oneOf`` over the kinds
       in ``step_vocabulary`` — each branch fixes ``step_type`` to a
       ``const`` and points ``fields`` at the kind's declared
       ``fields_schema``. Without this, the model treats ``fields`` as
       freeform and emits ``fields: {}`` even when keys are REQUIRED.

    2. The root object is replaced with a ``oneOf`` discriminated on
       ``kind``: one branch per proposal kind, each requiring the matching
       payload (``match`` / ``new`` / ``gap``) to be populated. Without
       this the model emits ``kind: 'new'`` with ``match: null``,
       ``new: null``, ``gap: null`` — the Pydantic root validator then
       rejects it for having no populated payload.
    """
    schema = copy.deepcopy(GraphProposal.model_json_schema())
    defs = schema.setdefault("$defs", {})
    branches = [_step_branch(kind) for kind in step_vocabulary]
    if branches:
        defs["ProposedStep"] = {
            "title": "ProposedStep",
            "description": (
                "One step in a New proposal. The step's `fields` payload is "
                "strictly typed against the step's kind — see the matching "
                "branch under oneOf."
            ),
            "oneOf": branches,
        }
    return _apply_top_level_discriminator(schema)


def _apply_top_level_discriminator(schema: dict[str, Any]) -> dict[str, Any]:
    """Replace the GraphProposal root with a kind-discriminated oneOf.

    Each branch sets the relevant payload (``match`` / ``new`` / ``gap``)
    as required and pins ``kind`` to a ``const`` so the model has to
    populate exactly one payload to satisfy the schema.
    """
    bundle_hash_schema = {
        "type": "string",
        "minLength": 8,
        "description": "Echo of the input bundle_hash for protocol integrity.",
    }
    return {
        "$defs": schema.get("$defs", {}),
        "title": schema.get("title", "GraphProposal"),
        "description": schema.get(
            "description",
            "Single artifact returned by the verify-graph-author worker.",
        ),
        "oneOf": [
            _root_branch("match", "MatchProposal", bundle_hash_schema),
            _root_branch("new", "NewProposal", bundle_hash_schema),
            _root_branch("gap", "GapProposal", bundle_hash_schema),
        ],
    }


def _root_branch(
    kind: str, payload_def: str, bundle_hash_schema: dict[str, Any]
) -> dict[str, Any]:
    """Build one top-level GraphProposal variant for a single ProposalKind."""
    return {
        "type": "object",
        "additionalProperties": False,
        "title": f"{kind} proposal",
        "properties": {
            "kind": {
                "type": "string",
                "const": kind,
                "description": f"Must be exactly '{kind}' for this variant.",
            },
            "bundle_hash": bundle_hash_schema,
            kind: {"$ref": f"#/$defs/{payload_def}"},
        },
        "required": ["kind", "bundle_hash", kind],
    }


def _step_branch(kind: StepKindRecord) -> dict[str, Any]:
    """Build one ProposedStep variant for a single step kind."""
    fields_schema = _normalise_fields_schema(kind.fields_schema)
    return {
        "type": "object",
        "additionalProperties": False,
        "title": f"{kind.kind} step",
        "description": (
            kind.description.strip() if kind.description else f"{kind.kind} step"
        ),
        "properties": {
            "step_type": {
                "type": "string",
                "const": kind.kind,
                "description": f"Must be exactly '{kind.kind}' for this variant.",
            },
            "fields": fields_schema,
            "provides_evidence_for": {
                "type": "array",
                "items": {"type": "string"},
                "description": "TC ids this step provides evidence for.",
            },
        },
        "required": ["step_type", "fields", "provides_evidence_for"],
    }


def _normalise_fields_schema(raw: Any) -> dict[str, Any]:
    """Ensure a kind's ``fields_schema`` is a usable JSON Schema object.

    The Rust side already populates per-kind schemas with ``type:object``,
    ``properties``, ``required``, ``additionalProperties``; this function
    is defensive — if a kind ships with an empty schema, we still emit
    a valid object schema rather than letting a stray ``{}`` slip through.
    """
    if not isinstance(raw, dict) or not raw:
        return {
            "type": "object",
            "additionalProperties": True,
            "description": "Payload for this step kind (no schema declared).",
        }
    return raw
