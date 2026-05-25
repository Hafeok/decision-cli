"""WireClient orchestrates SSE consumer, claim, completion poster, catalog cache."""

from __future__ import annotations

from collections.abc import AsyncIterator, Iterable
from dataclasses import dataclass

import httpx

from .catalog import CatalogCache
from .claim import ClaimClient
from .poster import CompletionPoster, RetryPolicy
from .sse import SseConsumer
from .types import (
    CapabilityCatalogEntry,
    ClaimResult,
    CompletionPayload,
    DispatchEvent,
)


@dataclass(frozen=True)
class HarnessEndpoints:
    """The four URLs the SDK needs from the harness.

    Most deployments will generate these from a single base URL; this
    structure keeps every URL explicit so test harnesses can route them
    independently to in-memory transports.
    """

    sse_url: str
    """SSE endpoint streaming dispatches (ADR-045)."""
    claim_url_template: str
    """POST endpoint for atomic claims; takes ``{dispatch_id}`` placeholder."""
    completion_url_template: str
    """POST endpoint for completions; takes ``{dispatch_id}`` placeholder."""
    catalog_url: str
    """GET endpoint returning the capability catalog (ADR-033)."""

    @classmethod
    def from_base(cls, base_url: str) -> HarnessEndpoints:
        base = base_url.rstrip("/")
        return cls(
            sse_url=f"{base}/events",
            claim_url_template=f"{base}/dispatches/{{dispatch_id}}/claim",
            completion_url_template=f"{base}/dispatches/{{dispatch_id}}/completion",
            catalog_url=f"{base}/catalog/capabilities",
        )


class WireClient:
    """The wire layer of the worker SDK: only thing in the SDK that does I/O.

    Wraps the four protocol surfaces (SSE consume, atomic claim, completion
    POST, catalog GET) behind a single async-iterable interface. The
    Session layer (FT-078) drives it; the WireClient itself stays
    transport-pure (no graph access, no model calls, no business logic).
    """

    def __init__(
        self,
        worker_id: str,
        capability_tags: Iterable[str],
        endpoints: HarnessEndpoints,
        *,
        client: httpx.AsyncClient | None = None,
        retry_policy: RetryPolicy | None = None,
        catalog_ttl_seconds: float = 300.0,
    ) -> None:
        self.worker_id = worker_id
        self.capability_tags = frozenset(capability_tags)
        self.endpoints = endpoints
        self._client = client
        self._owns_client = client is None

        self.sse = SseConsumer(endpoints.sse_url, self.capability_tags, client=client)
        self.claim_client = ClaimClient(
            endpoints.claim_url_template, worker_id, client=client
        )
        self.poster = CompletionPoster(
            endpoints.completion_url_template,
            client=client,
            policy=retry_policy,
        )
        self.catalog = CatalogCache(
            endpoints.catalog_url, client=client, ttl_seconds=catalog_ttl_seconds
        )

    async def aclose(self) -> None:
        # Close each subcomponent first so they release any borrowed
        # references before we drop the shared client.
        await self.sse.aclose()
        await self.claim_client.aclose()
        await self.poster.aclose()
        await self.catalog.aclose()
        if self._owns_client and self._client is not None:
            await self._client.aclose()
            self._client = None

    @property
    def last_event_id(self) -> str | None:
        """Most recent dispatched event id — survives across reconnects."""
        return self.sse.last_event_id

    def set_last_event_id(self, event_id: str | None) -> None:
        """Override the SSE resume cursor (warm-start from a checkpoint)."""
        self.sse.set_last_event_id(event_id)

    async def dispatches(self) -> AsyncIterator[DispatchEvent]:
        """Stream dispatch events from the harness.

        On disconnect, callers should re-enter this method; the SSE
        consumer already knows the resume cursor.
        """
        async for dispatch in self.sse.stream():
            yield dispatch

    async def claim(self, dispatch_id: str) -> ClaimResult:
        """Attempt to claim a dispatch atomically."""
        return await self.claim_client.claim(dispatch_id)

    async def complete(self, payload: CompletionPayload) -> httpx.Response:
        """POST a completion payload; raises on rejection/exhaustion."""
        return await self.poster.post(payload)

    async def resolve_capability(
        self, capability_tag: str
    ) -> CapabilityCatalogEntry | None:
        """Resolve a capability tag through the per-process catalog cache."""
        return await self.catalog.resolve(capability_tag)
