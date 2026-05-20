"""CLI entry point — read VerifierInput from stdin, write VerificationVerdict to stdout.

ADR-008 / ADR-020 worker contract:

* Exit 0 iff a Pydantic-validated VerificationVerdict was emitted on
  stdout.
* Exit 1 on bundle parse error, Claude error, or validation failure
  (with a diagnostic on stderr).
* No files written, no graph access, no subprocess plumbing beyond the
  single anthropic SDK call.
"""

from __future__ import annotations

import argparse
import json
import sys
from typing import Sequence

from pydantic import ValidationError

from .bundle import VerifierInput
from .worker import VerifierError, run_verifier


def _parse_args(argv: Sequence[str] | None) -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        prog="verifier",
        description=(
            "Stateless Python worker for decision-cli's verifier role. "
            "Bundle in, VerificationVerdict artifact out (ADR-008, ADR-020)."
        ),
    )
    parser.add_argument(
        "--include-telemetry",
        action="store_true",
        help="Emit telemetry as a second JSON object on stderr (for harness diagnostics).",
    )
    return parser.parse_args(argv)


def main(argv: Sequence[str] | None = None) -> int:
    args = _parse_args(argv)
    raw = sys.stdin.read()
    if not raw.strip():
        sys.stderr.write(
            "verifier: bundle parse error: empty stdin — expected a VerifierInput JSON document\n"
        )
        return 1
    try:
        bundle = VerifierInput.model_validate_json(raw)
    except ValidationError as exc:
        sys.stderr.write(f"verifier: bundle parse error: {exc}\n")
        return 1

    try:
        result = run_verifier(bundle)
    except VerifierError as exc:
        message = str(exc)
        if "below minimum length" in message or "minLength" in message or "20 characters" in message:
            sys.stderr.write(f"verifier: rationale below minimum length\n")
        else:
            sys.stderr.write(f"verifier: model call failed: {message}\n")
        return 1
    except Exception as exc:  # noqa: BLE001 - bubble any unexpected failure with detail
        sys.stderr.write(f"verifier: model call failed: {exc}\n")
        return 1

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
