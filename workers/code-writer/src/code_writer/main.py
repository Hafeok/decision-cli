"""Entry point for the code-writer worker.

Slice 1 contract (FT-013 / ADR-008):

* **One-shot mode (default for the harness):** reads a single
  ``DispatchPayload`` JSON document from stdin, runs the dispatch,
  writes the resulting ``WorkerResponse`` JSON to stdout. This is the
  path ``dec implement`` (FT-011) drives end-to-end.
* **Daemon mode (``--sse-url``):** subscribes to the FT-004 SSE
  endpoint, processes dispatches one at a time, and prints each
  response to stdout. Slice 1 does not yet write responses back to the
  harness over SSE — the harness uses one-shot mode for the closed
  loop. Daemon mode exists to demonstrate the FT-013 §Behaviour 1-2
  subscription path the contract requires.

The worker honours the ``CODE_WRITER_STUB`` env var (or ``--stub`` CLI
flag): when set, dispatches are handled by the deterministic stub
runner. This keeps TC-008 / TC-013 reproducible without depending on a
Claude Code subscription session.
"""

from __future__ import annotations

import argparse
import json
import sys
from typing import Sequence

from pydantic import ValidationError

from .claude_runner import run_dispatch
from .models import DispatchPayload, WorkerError, WorkerResponse
from .sse import stream_dispatches


def _parse_args(argv: Sequence[str] | None) -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        prog="code-writer",
        description=(
            "Stateless Python worker for decision-cli's implementer role. "
            "Bundle in, CodeChange artifact out (ADR-008)."
        ),
    )
    sub = parser.add_subparsers(dest="command")

    p_once = sub.add_parser(
        "run-once",
        help="Read a DispatchPayload from stdin, write a WorkerResponse to stdout.",
    )
    p_once.add_argument(
        "--stub",
        action="store_true",
        help="Force stub mode (overrides CODE_WRITER_STUB env var).",
    )
    p_once.add_argument(
        "--no-stub",
        action="store_true",
        help="Force real `claude -p` mode (overrides CODE_WRITER_STUB env var).",
    )

    p_daemon = sub.add_parser(
        "daemon",
        help="Subscribe to the harness SSE endpoint and process dispatches.",
    )
    p_daemon.add_argument(
        "--sse-url",
        required=True,
        help="SSE endpoint exposed by `dec` (e.g. http://127.0.0.1:7878/events).",
    )
    p_daemon.add_argument("--stub", action="store_true")
    p_daemon.add_argument("--no-stub", action="store_true")

    return parser.parse_args(argv)


def _force_stub_from_args(args: argparse.Namespace) -> bool | None:
    if args.stub and args.no_stub:
        raise SystemExit("--stub and --no-stub are mutually exclusive")
    if args.stub:
        return True
    if args.no_stub:
        return False
    return None


def _run_once(args: argparse.Namespace) -> int:
    raw = sys.stdin.read()
    if not raw.strip():
        err = WorkerError(
            category="invalid_dispatch",
            message="empty stdin — expected a DispatchPayload JSON document",
        )
        json.dump(_error_response("", "", err), sys.stdout)
        sys.stdout.write("\n")
        return 2
    try:
        payload = DispatchPayload.model_validate_json(raw)
    except ValidationError as exc:
        err = WorkerError(
            category="invalid_dispatch",
            message="DispatchPayload failed validation",
            detail=str(exc),
        )
        json.dump(_error_response("", "", err), sys.stdout)
        sys.stdout.write("\n")
        return 2
    response = run_dispatch(payload, force_stub=_force_stub_from_args(args))
    sys.stdout.write(response.model_dump_json())
    sys.stdout.write("\n")
    sys.stdout.flush()
    return 0 if response.status == "ok" else 1


def _run_daemon(args: argparse.Namespace) -> int:
    force_stub = _force_stub_from_args(args)
    for envelope in stream_dispatches(args.sse_url):
        # Slice 1: the dispatch envelope is the same shape as the SSE
        # `EventEnvelope` — it points at an oxi:Event in the graph but
        # does NOT carry the full DispatchPayload yet. We log the
        # event and skip; harness wiring (FT-011) will be enhanced to
        # carry the payload inline in slice 2 (or fetched via product
        # MCP). For slice 1, the harness uses one-shot mode for the
        # closed loop and daemon mode is a smoke-test surface.
        sys.stderr.write(f"daemon: received dispatch envelope {envelope!r}\n")
        # Forwarding stub: parse only if the envelope carries a payload
        # field (future-proof).
        payload_raw = envelope.get("payload")
        if not payload_raw:
            continue
        try:
            payload = DispatchPayload.model_validate(payload_raw)
        except ValidationError as exc:
            sys.stderr.write(f"daemon: skip malformed payload: {exc}\n")
            continue
        response = run_dispatch(payload, force_stub=force_stub)
        sys.stdout.write(response.model_dump_json())
        sys.stdout.write("\n")
        sys.stdout.flush()
    return 0


def _error_response(dispatch_id: str, session_id: str, err: WorkerError) -> dict:
    return WorkerResponse(
        dispatch_id=dispatch_id,
        session_id=session_id,
        status="error",
        error=err,
    ).model_dump()


def main(argv: Sequence[str] | None = None) -> int:
    args = _parse_args(argv)
    if args.command == "run-once":
        return _run_once(args)
    if args.command == "daemon":
        return _run_daemon(args)
    sys.stderr.write(
        "code-writer: no subcommand given. Try `code-writer run-once` or "
        "`code-writer daemon --sse-url <url>`.\n"
    )
    return 2


if __name__ == "__main__":
    raise SystemExit(main())
