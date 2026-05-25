"""Per-process cache of the harness model catalog (FT-077, ADR-033)."""

from __future__ import annotations

import time
from collections.abc import Callable, Iterable

import httpx

from .types import CapabilityCatalogEntry


class CatalogCache:
    """Lazy capability-tag → model-group cache for one worker process.

    The harness's capability catalog (ADR-036) is a slow-moving graph
    artifact — bindings change orders of magnitude less often than
    dispatches. Re-fetching the catalog on every dispatch is pure waste;
    once the worker process has the mapping it needs, it should reuse it
    for the lifetime of the process unless the operator explicitly
    invalidates the entry.

    The cache is per-process, not per-thread: every consumer in the same
    worker shares the same cached view, and an explicit ``invalidate()``
    flushes it.
    """

    def __init__(
        self,
        catalog_url: str,
        *,
        client: httpx.AsyncClient | None = None,
        ttl_seconds: float = 300.0,
        clock: Callable[[], float] = time.monotonic,
        timeout: float = 10.0,
    ) -> None:
        self.catalog_url = catalog_url
        self._client = client
        self._owns_client = client is None
        self._entries: dict[str, CapabilityCatalogEntry] = {}
        self._fetched_at: float | None = None
        self._ttl = ttl_seconds
        self._clock = clock
        self._timeout = timeout

    async def _client_or_new(self) -> httpx.AsyncClient:
        if self._client is None:
            self._client = httpx.AsyncClient(timeout=self._timeout)
        return self._client

    async def aclose(self) -> None:
        if self._owns_client and self._client is not None:
            await self._client.aclose()
            self._client = None

    def _is_fresh(self) -> bool:
        if self._fetched_at is None:
            return False
        return (self._clock() - self._fetched_at) < self._ttl

    def invalidate(self) -> None:
        """Drop the cached entries; next ``resolve`` call refetches."""
        self._entries = {}
        self._fetched_at = None

    def prime(self, entries: Iterable[CapabilityCatalogEntry]) -> None:
        """Seed the cache from a known-good catalog snapshot.

        Useful for warm starts and for tests that bypass HTTP. Refreshes
        the cached-at timestamp so the seeded view is treated as fresh.
        """
        self._entries = {e.capability_tag: e for e in entries}
        self._fetched_at = self._clock()

    async def _fetch(self) -> None:
        client = await self._client_or_new()
        response = await client.get(self.catalog_url)
        response.raise_for_status()
        body = response.json()
        raw_entries = body.get("entries", body) if isinstance(body, dict) else body
        entries: dict[str, CapabilityCatalogEntry] = {}
        if isinstance(raw_entries, list):
            for raw in raw_entries:
                entry = CapabilityCatalogEntry.model_validate(raw)
                entries[entry.capability_tag] = entry
        self._entries = entries
        self._fetched_at = self._clock()

    async def resolve(self, capability_tag: str) -> CapabilityCatalogEntry | None:
        """Return the catalog entry for ``capability_tag`` (or None if unknown).

        Triggers one HTTP fetch on cold start or after TTL expiry; later
        calls hit the in-memory map. A miss after a fresh fetch returns
        ``None`` so the caller can surface "unknown capability tag" as
        feedback rather than retrying the catalog blindly.
        """
        if not self._is_fresh():
            await self._fetch()
        return self._entries.get(capability_tag)

    async def all(self) -> dict[str, CapabilityCatalogEntry]:
        """Return the cached view of the catalog, refreshing if stale."""
        if not self._is_fresh():
            await self._fetch()
        return dict(self._entries)
