"""HTTP poster for completion events with bounded exponential backoff (ADR-045)."""

from __future__ import annotations

import asyncio
import random
from dataclasses import dataclass

import httpx

from .types import CompletionPayload


@dataclass(frozen=True)
class RetryPolicy:
    """Bounded-retry policy for transient completion failures.

    Slice 1 retries only on connection / 5xx errors; 4xx responses are
    terminal (the harness's SHACL validation report is the result). The
    backoff is full-jitter exponential capped at ``max_backoff``.
    """

    max_attempts: int = 5
    base_backoff: float = 0.25
    max_backoff: float = 8.0

    def sleep_for(self, attempt: int) -> float:
        # attempt is 1-indexed; first retry uses base_backoff
        exp = min(self.max_backoff, self.base_backoff * (2 ** (attempt - 1)))
        return random.uniform(0, exp)


class CompletionRejected(Exception):
    """The harness rejected the completion with a 4xx; do not retry."""

    def __init__(self, status: int, body: str) -> None:
        super().__init__(f"completion rejected: HTTP {status}: {body[:200]}")
        self.status = status
        self.body = body


class CompletionFailed(Exception):
    """All retries exhausted on a transient failure path."""

    def __init__(self, attempts: int, last_error: str) -> None:
        super().__init__(
            f"completion failed after {attempts} attempts; last error: {last_error}"
        )
        self.attempts = attempts
        self.last_error = last_error


def _is_retryable_status(status: int) -> bool:
    return status >= 500 or status == 429


class CompletionPoster:
    """POST completion payloads to the harness with retry/backoff.

    Per ADR-045 the wire is HTTP POST to a per-dispatch endpoint. The
    poster wraps the request in a bounded retry loop so transient
    network or 5xx failures do not lose work; deterministic 4xx
    rejections (e.g. SHACL failure) raise ``CompletionRejected`` so the
    Session layer can surface the validation report.
    """

    def __init__(
        self,
        endpoint_template: str,
        *,
        client: httpx.AsyncClient | None = None,
        policy: RetryPolicy | None = None,
        timeout: float = 30.0,
        sleep_fn=asyncio.sleep,
    ) -> None:
        """
        ``endpoint_template`` is a format string with a ``{dispatch_id}``
        placeholder; the poster substitutes the dispatch id at call time.
        Falls back to the literal string if no placeholder is present
        (caller has pre-composed the URL).
        """
        self.endpoint_template = endpoint_template
        self._client = client
        self._owns_client = client is None
        self._policy = policy or RetryPolicy()
        self._timeout = timeout
        self._sleep = sleep_fn

    async def _client_or_new(self) -> httpx.AsyncClient:
        if self._client is None:
            self._client = httpx.AsyncClient(timeout=self._timeout)
        return self._client

    async def aclose(self) -> None:
        if self._owns_client and self._client is not None:
            await self._client.aclose()
            self._client = None

    def _url_for(self, dispatch_id: str) -> str:
        if "{dispatch_id}" in self.endpoint_template:
            return self.endpoint_template.format(dispatch_id=dispatch_id)
        return self.endpoint_template

    async def post(self, payload: CompletionPayload) -> httpx.Response:
        """Send the completion, retrying transient failures.

        Returns the harness's success response on a 2xx, raises
        ``CompletionRejected`` on a deterministic 4xx, or
        ``CompletionFailed`` once retries are exhausted.
        """
        client = await self._client_or_new()
        url = self._url_for(payload.dispatch_id)
        body = payload.model_dump(mode="json")
        last_error = ""
        for attempt in range(1, self._policy.max_attempts + 1):
            try:
                response = await client.post(url, json=body)
            except httpx.HTTPError as exc:
                last_error = f"{type(exc).__name__}: {exc}"
            else:
                if 200 <= response.status_code < 300:
                    return response
                if 400 <= response.status_code < 500 and not _is_retryable_status(
                    response.status_code
                ):
                    raise CompletionRejected(response.status_code, response.text)
                last_error = f"HTTP {response.status_code}: {response.text[:200]}"
            if attempt < self._policy.max_attempts:
                await self._sleep(self._policy.sleep_for(attempt))
        raise CompletionFailed(self._policy.max_attempts, last_error)
