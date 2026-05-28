---
id: ADR-009
title: product-cli integration via subprocess and MCP for slice 1
status: accepted
features:
- FT-105
supersedes: []
superseded-by: []
domains: []
scope: feature-specific
content-hash: sha256:e249f54eb925330c1b3f2f76fe6ba127abe1762ad98fb68332742cd5ba8f0c99
---

## Context

decision-cli orchestrates product-cli. product-cli already exposes:

- A CLI (`product feature show`, `product context FT-XXX --depth N`, `product preflight FT-XXX`, etc.) with structured stdout (JSON where supported).
- An MCP server (stdio and HTTP) exposing the same surface as tools.

The integration question for slice 1: how does decision-cli read from and write to product-cli?

Two approaches:

1. **Library integration.** Embed product-cli as a Rust library; call its API in-process. Tight coupling, requires product-cli to expose a stable library API.
2. **Service integration.** Treat product-cli as a service consumed via its existing interfaces (subprocess CLI for reads; MCP write tools for writes). Loose coupling, uses the same interfaces humans use.

product-cli's own PRD says *"Product does not invoke agents"* — it is deliberately layered below decision-cli. Forcing a library integration would couple their release cycles and require product-cli to expose API surface it doesn't otherwise need. Service integration also realises the DDD principle: *"single interface for humans and LLMs"* — decision-cli uses exactly the surface humans use.

See `decision-cli-slice-1-bounds.md` §4, §8.

## Decision

For slice 1, the integration is **one-directional and service-based**:

- **Reads:** subprocess invocation of `product feature next`, `product feature show`, `product context FT-XXX --depth N`, `product preflight FT-XXX`, `product graph stats`. Output is parsed from stdout (JSON where supported, structured text otherwise).
- **Writes:** invocation of product-cli's **MCP write tools** for `CodeChange` registration. Slice 1 may extend product-cli with a minimal new artifact type (`CodeChange`) if a feature_spec for that emerges from authoring.
- **No event subscription:** product-cli stays oblivious to decision-cli. When decision-cli needs to act on a product-cli state change, the human triggers it via `dec implement FT-XXX` (see ADR-010).

product-cli remains oblivious to decision-cli's existence. Wiring product-cli to **emit** events through oxi-events is slice 2 work.

## Consequences

**Positive:**

- product-cli evolves independently; decision-cli adapts to its CLI/MCP surface without coordinating releases.
- The integration uses the same surfaces humans use — every CLI invocation by the orchestrator is reproducible by a human.
- Decoupling protects product-cli from accidentally absorbing DDD concepts.
- Failure modes are clear: a subprocess exits non-zero, MCP returns a structured error.

**Negative / accepted costs:**

- Subprocess overhead per read (acceptable for slice 1's volume).
- stdout parsing requires robust handling of mixed JSON/text formats.
- No reactive loop in slice 1 — every dispatch starts from explicit human trigger (see ADR-010).

**Slice 2 changes:**

- product-cli wired to emit oxi-events events (the full reactive loop).
- Possibly direct library integration for performance-critical paths once the boundary is well-tested.

## Status

Accepted. Governs FT-011 (implementer role end-to-end), with slice 1 scope explicit in §6.2 (deferred items).
