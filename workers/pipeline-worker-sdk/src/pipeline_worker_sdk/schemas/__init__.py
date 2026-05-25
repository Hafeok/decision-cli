"""Schemas layer — Pydantic models generated from SHACL output shapes.

These models live under :mod:`pipeline_worker_sdk.schemas._generated` and are
suitable for use with LiteLLM / instructor structured output (the worker
SDK's primary consumer). One model per artifact type, with required-field
metadata sourced from the same SHACL declarations the harness validates.
"""

from . import _generated as models  # noqa: F401

__all__ = ["models"]
