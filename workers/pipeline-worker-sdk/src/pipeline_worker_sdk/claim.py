"""Atomic claim mechanism for multi-worker capability-tag contention (FT-077)."""

from __future__ import annotations

import httpx

from .types import ClaimResult


class ClaimClient:
    """POST a claim on a dispatch; exactly one concurrent caller wins.

    Multiple workers advertising the same capability tag receive the same
    dispatch envelope over their respective SSE streams. To prevent
    duplicate work, each worker must POST to a per-dispatch claim
    endpoint before starting. The harness atomically returns 200 (won) to
    the first claimer and 409 (lost / already-claimed) to the rest.

    The claim endpoint is idempotent within one worker process: a second
    claim from the same worker id on the same dispatch returns ``won=True``
    so retries after a transient failure do not strand work.
    """

    def __init__(
        self,
        endpoint_template: str,
        worker_id: str,
        *,
        client: httpx.AsyncClient | None = None,
        timeout: float = 10.0,
    ) -> None:
        self.endpoint_template = endpoint_template
        self.worker_id = worker_id
        self._client = client
        self._owns_client = client is None
        self._timeout = timeout

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

    async def claim(self, dispatch_id: str) -> ClaimResult:
        """Attempt to claim the dispatch.

        Returns a ``ClaimResult`` with ``won=True`` if this worker owns
        the dispatch, ``won=False`` otherwise. Any non-409 error is
        propagated as the exception that produced it (callers treat
        propagation as "do not run this dispatch and surface the error").
        """
        client = await self._client_or_new()
        url = self._url_for(dispatch_id)
        try:
            response = await client.post(
                url, json={"dispatch_id": dispatch_id, "worker_id": self.worker_id}
            )
        except httpx.HTTPError:
            # Treat a failed claim attempt as a loss with the network
            # error as the reason — never run a dispatch you could not
            # confirm you own.
            return ClaimResult(dispatch_id=dispatch_id, won=False, reason="network-error")

        if 200 <= response.status_code < 300:
            return ClaimResult(dispatch_id=dispatch_id, won=True)
        if response.status_code == 409:
            try:
                body = response.json()
                reason = body.get("reason", "already-claimed")
            except (ValueError, TypeError):
                reason = "already-claimed"
            return ClaimResult(dispatch_id=dispatch_id, won=False, reason=str(reason))
        # Other unexpected status: refuse the dispatch but surface why.
        return ClaimResult(
            dispatch_id=dispatch_id,
            won=False,
            reason=f"unexpected-status-{response.status_code}",
        )
