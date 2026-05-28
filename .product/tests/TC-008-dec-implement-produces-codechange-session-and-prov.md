---
id: TC-008
title: dec_implement_produces_codechange_session_and_provenance
type: exit-criteria
status: passing
validates:
  features: []
  adrs: []
phase: 1
runner: bash
runner-args: tests/scripts/tc-008-implement-e2e.sh
runner-timeout: 180
last-run: 2026-05-28T09:35:03.993499834+00:00
last-run-duration: 0.7s
---

## Purpose

End-to-end exit criterion for the implementer slice (FT-011 + FT-013): a single `dec implement FT-XXX` invocation must produce a `CodeChange` registered in product-cli's graph **and** a `Session` record in decision-cli's graph, both linked by PROV-O (**ADR-004**), with the Session linked to the active ValueStream via `dec:inStream` (**ADR-005**). product-cli is consumed as a service (**ADR-009**); the worker follows the stateless contract (**ADR-008**); the trigger is explicit human action (**ADR-010**).

Source: `decision-cli-slice-1-bounds.md` §11.2 exit-criteria #8.

## Given

- A working directory initialized via `dec init` (FT-008).
- product-cli installed and reachable as a subprocess (`product` on PATH).
- A target feature `FT-XXX` exists in product-cli's graph, ready for implementation, with a meaningful context bundle.
- The Python code-writer worker (FT-013) is running and subscribed to the dispatch event channel.
- Claude Code CLI (`claude`) installed and reachable as a subprocess with an authenticated subscription session on the host (run `claude login` once during operator setup). No `ANTHROPIC_API_KEY` is required.

## When

```bash
dec implement FT-XXX
```

## Then

1. The command exits 0.
2. **In decision-cli's graph**, there exists one new `Session` artifact such that:
   - It carries `dec:inStream <decision-cli-development>` (ADR-005, validated globally by TC-014).
   - It is `prov:Activity`-typed; `prov:used` references the bundle (with its hash) and the model id.
   - The dispatch event PROV chain resolves backward to this Session.
3. **In product-cli's graph**, there exists one new `CodeChange` artifact such that:
   - It is reachable from the decision-cli Session via PROV-O lineage (`CodeChange prov:wasGeneratedBy Session` or equivalent), satisfying TC-013.
4. At least one file appears in the workspace under the worker's configured workspace path, corresponding to the `CodeChange`'s declared file list.
5. `dec session show <id>` (FT-012) reports the same Session with bundle hash, model version, and output reference.

## Notes

- Combines TC-012, TC-013, TC-014 in a single end-to-end run; this TC is the integration smoke test those invariants harden.
- TC-011 separately verifies the SSE delivery latency component.
- Test fixture must include a stub or canned product-cli installation; the TC runner is `bash` for now (test-runner config can be added when test fixtures land).