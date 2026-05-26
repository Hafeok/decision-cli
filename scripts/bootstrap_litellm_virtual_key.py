#!/usr/bin/env python3
"""Issue a LiteLLM virtual key scoped to configured model groups (FT-096).

Calls LiteLLM's `/key/generate` endpoint with the master key, then writes
the issued virtual key into the operator's `workers.env` so the
`pipeline-cli workers run` subcommand (FT-095) can inject it into worker
containers as `LITELLM_API_KEY`. Provider API keys never leave LiteLLM's
process — workers see only the scoped virtual key.

Slice 1 default: one shared virtual key per operator, scoped to the
model groups declared in `config/litellm.yaml`. Per-WorkerImage virtual
keys are a slice-2 progression per FT-096's "Out of scope".

Usage:

    python scripts/bootstrap_litellm_virtual_key.py \\
        --litellm-base-url http://localhost:4000 \\
        --models frontier-reasoning,fast-cheap \\
        --workers-env ~/.pipeline-cli/workers.env

Required env: `LITELLM_MASTER_KEY` (the proxy's master key).
"""

from __future__ import annotations

import argparse
import json
import os
import sys
import urllib.request
from pathlib import Path

DEFAULT_LITELLM_BASE_URL = "http://localhost:4000"
DEFAULT_WORKERS_ENV = Path.home() / ".pipeline-cli" / "workers.env"
DEFAULT_MODELS = "frontier-reasoning,fast-cheap"
DEFAULT_BUDGET_USD = 25.0
MASTER_KEY_ENV = "LITELLM_MASTER_KEY"


def main() -> int:
    args = parse_args()
    master_key = os.environ.get(MASTER_KEY_ENV)
    if not master_key:
        sys.stderr.write(f"bootstrap_litellm_virtual_key: {MASTER_KEY_ENV} not set\n")
        return 1
    models = [m.strip() for m in args.models.split(",") if m.strip()]
    if not models:
        sys.stderr.write("bootstrap_litellm_virtual_key: --models must be non-empty\n")
        return 1
    issued = issue_virtual_key(
        base_url=args.litellm_base_url,
        master_key=master_key,
        models=models,
        budget_usd=args.budget_usd,
    )
    write_workers_env(
        path=args.workers_env,
        litellm_base_url=args.litellm_base_url,
        litellm_api_key=issued,
    )
    sys.stdout.write(
        f"bootstrap_litellm_virtual_key: wrote virtual key to {args.workers_env}\n"
    )
    return 0


def parse_args() -> argparse.Namespace:
    p = argparse.ArgumentParser(
        prog="bootstrap_litellm_virtual_key.py",
        description=(
            "Issue a LiteLLM virtual key via /key/generate and persist it "
            "into workers.env for pipeline-cli workers run."
        ),
    )
    p.add_argument("--litellm-base-url", default=DEFAULT_LITELLM_BASE_URL)
    p.add_argument("--models", default=DEFAULT_MODELS)
    p.add_argument("--budget-usd", type=float, default=DEFAULT_BUDGET_USD)
    p.add_argument("--workers-env", type=Path, default=DEFAULT_WORKERS_ENV)
    return p.parse_args()


def issue_virtual_key(
    base_url: str,
    master_key: str,
    models: list[str],
    budget_usd: float,
) -> str:
    url = base_url.rstrip("/") + "/key/generate"
    payload = json.dumps({"models": models, "max_budget": budget_usd}).encode("utf-8")
    request = urllib.request.Request(  # noqa: S310 - trusted local proxy
        url,
        data=payload,
        method="POST",
        headers={
            "content-type": "application/json",
            "authorization": f"Bearer {master_key}",
        },
    )
    with urllib.request.urlopen(request, timeout=10) as resp:  # noqa: S310
        body = json.loads(resp.read().decode("utf-8"))
    key = body.get("key")
    if not key:
        raise RuntimeError(
            f"LiteLLM /key/generate did not return a 'key' field: {body!r}"
        )
    return str(key)


def write_workers_env(
    path: Path,
    litellm_base_url: str,
    litellm_api_key: str,
) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    existing = {}
    if path.exists():
        for line in path.read_text(encoding="utf-8").splitlines():
            stripped = line.strip()
            if not stripped or stripped.startswith("#") or "=" not in stripped:
                continue
            key, _, value = stripped.partition("=")
            existing[key.strip()] = value.strip()
    existing["LITELLM_BASE_URL"] = litellm_base_url
    existing["LITELLM_API_KEY"] = litellm_api_key
    body_lines = [f"{k}={v}" for k, v in sorted(existing.items())]
    path.write_text("\n".join(body_lines) + "\n", encoding="utf-8")


if __name__ == "__main__":
    sys.exit(main())
