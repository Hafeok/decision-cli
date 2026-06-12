---
id: TC-469
title: 'language capability gates the semantic surface: no catalog entry means no tools, handshake mismatch drops exactly the unadvertised tool'
type: scenario
status: unimplemented
validates:
  features: []
  adrs: []
phase: 1
runner: cargo-test
runner-args: -p decision-cli ft_179_language_capability_gate
runner-timeout: 300
observes:
- graph
- stdout
---

## Description

Three assertions over the ADR-092 §8/§9 capability gate:

1. A dispatch targeting only files whose language has no `dec:LanguageServer` catalog entry (fixture `.ttl` cell) exposes no semantic tools — the payload's effective surface contains none of the six, asserted on the **graph** (catalog quads) and **stdout** (the stub worker's recorded registry).
2. With a fixture server whose `initialize` response omits a declared capability (no `referencesProvider`), exactly `find_references` is dropped from the surface; the other five remain, and the mismatch is recorded in telemetry (**stdout**).
3. The resolved support matrix (catalog joined with the last handshake result) is queryable from the **graph** after the run.
