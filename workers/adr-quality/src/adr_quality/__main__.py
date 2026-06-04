"""CLI entry point — read AdrQualityInput from a path, write QualityVerdict to stdout.

FT-133 / ADR-073 worker contract:

* Invocation: ``python -m adr_quality --bundle <path-to-input-json>`` or
  ``python -m adr_quality --stdin``
* Exit 0 iff a Pydantic-validated QualityVerdict was emitted on stdout
  (regardless of `verdict`: approved, rejected, amendment-required are all valid outcomes).
* Exit codes for infrastructure faults per FT-133's error table:
    - 2: bundle parse error (missing required field or malformed JSON)
    - 3: model call failure (network, auth, rate limit)
    - 4: schema validation failure on Claude response after one retry
    - 5: QualityVerdict.bundle_hash ≠ input bundle_hash (protocol bug)
* No files written, no graph access, no subprocess plumbing beyond the
  single anthropic SDK call.
"""

from __future__ import annotations

import argparse
import json
import re
import sys
from pathlib import Path
from typing import Sequence

from pydantic import ValidationError

from .bundle import AdrQualityInput
from .worker import BundleHashMismatch, WorkerError, run_judge

_BUNDLE_HASH_RE = re.compile(r"^[A-Za-z0-9_-]{8,}$")


def _parse_args(argv: Sequence[str] | None) -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        prog="adr-quality",
        description=(
            "Stateless Python worker for decision-cli's adr-quality role. "
            "Bundle in, QualityVerdict artifact out (ADR-073, FT-133)."
        ),
    )
    src = parser.add_mutually_exclusive_group()
    src.add_argument(
        "--bundle",
        type=Path,
        default=None,
        help="Path to an AdrQualityInput JSON document (FT-133 default invocation).",
    )
    src.add_argument(
        "--stdin",
        action="store_true",
        help="Read the bundle from stdin instead of --bundle.",
    )
    parser.add_argument(
        "--include-telemetry",
        action="store_true",
        help="Emit telemetry as a second JSON object on stderr (for harness diagnostics).",
    )
    return parser.parse_args(argv)


def _read_bundle_source(args: argparse.Namespace) -> tuple[str | None, str | None]:
    """Return (raw_text, error_message). One of them is always None."""
    if args.bundle is not None:
        path: Path = args.bundle
        try:
            return path.read_text(encoding="utf-8"), None
        except FileNotFoundError:
            return None, f"bundle file not found: {path}"
        except OSError as exc:
            return None, f"bundle file unreadable ({path}): {exc}"
    raw = sys.stdin.read()
    if not raw.strip():
        return None, "empty stdin — pass --bundle <path> or pipe an AdrQualityInput JSON"
    return raw, None


def main(argv: Sequence[str] | None = None) -> int:
    args = _parse_args(argv)

    raw, read_err = _read_bundle_source(args)
    if read_err is not None:
        sys.stderr.write(f"adr-quality: bundle parse error: {read_err}\n")
        return 2

    try:
        bundle = AdrQualityInput.model_validate_json(raw)
    except ValidationError as exc:
        sys.stderr.write(f"adr-quality: bundle parse error: {exc}\n")
        return 2

    if not _BUNDLE_HASH_RE.match(bundle.bundle_hash):
        sys.stderr.write(
            "adr-quality: bundle parse error: bundle_hash is malformed "
            "(expected hex-like >= 8 chars)\n"
        )
        return 2

    try:
        result = run_judge(bundle)
    except BundleHashMismatch as exc:
        sys.stderr.write(f"adr-quality: bundle_hash mismatch: {exc}\n")
        return 5
    except WorkerError as exc:
        message = str(exc)
        if "failed validation after retry" in message:
            sys.stderr.write(f"adr-quality: schema validation failed: {message}\n")
            return 4
        sys.stderr.write(f"adr-quality: model call failed: {message}\n")
        return 3
    except Exception as exc:  # noqa: BLE001 - any unexpected failure
        sys.stderr.write(f"adr-quality: model call failed: {exc}\n")
        return 3

    sys.stdout.write(result.verdict.model_dump_json(exclude_none=True))
    sys.stdout.write("\n")
    sys.stdout.flush()

    if args.include_telemetry:
        sys.stderr.write(json.dumps(_telemetry_dict(result.telemetry)) + "\n")
    return 0


def _telemetry_dict(t) -> dict:
    return {
        "model_id": t.model_id,
        "input_tokens": t.input_tokens,
        "output_tokens": t.output_tokens,
        "latency_seconds": round(t.latency_seconds, 6),
        "attempts": t.attempts,
        "exit_reason": t.exit_reason,
    }


if __name__ == "__main__":
    raise SystemExit(main())
