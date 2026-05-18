---
id: TC-010
title: outbox_resumes_in_flight_dispatch_after_crash
type: chaos
status: failing
validates:
  features: []
  adrs: []
phase: 1
runner: bash
runner-args: tests/scripts/tc-010-outbox-recovery.sh
runner-timeout: 180
---

## Purpose

Chaos test for **ADR-002** outbox crash recovery (FT-003): if `dec` is killed mid-dispatch with at least one event marked `published = false`, restarting must resume delivery — the outbox publisher must re-publish on startup via the SPARQL scan described in FT-003.

Source: `decision-cli-slice-1-bounds.md` §11.2 exit-criteria #10.

## Given

- A running `dec` instance mid-dispatch (e.g., a `dec implement FT-XXX` in flight; FT-011).
- A subscribed consumer (e.g., the Python worker; FT-013) on SSE (FT-004) that has **not yet** acknowledged the dispatch event.
- The orchestration store on disk per FT-009.

## When

1. `kill -9 <dec pid>` (no graceful shutdown).
2. Restart `dec` against the same working directory.

## Then

1. On startup, `dec` opens the store and the FT-003 outbox publisher's scan
   ```sparql
   SELECT ?e WHERE { ?e a oxi:Event ; oxi:published false }
   ```
   returns the in-flight event(s).
2. Each unpublished event is re-published to the configured transports.
3. The subscribed consumer (re-subscribing via SSE) receives the event within a bounded interval and proceeds to complete the dispatch.
4. After successful re-delivery, the event's `oxi:published` flag flips to `true` in the store.

## Notes

- The runner should be `bash` driving a process supervisor; the SIGKILL semantics are required (a clean shutdown would not exercise the recovery path).
- TC-009 establishes the precondition that events are durably persisted; this TC validates the resumption semantics.

## Formal specification

⟦Σ:Types⟧{
  EventId ≜ IRI
  Seq ≜ ℕ
  Event ≜ ⟨id:EventId, seq:Seq, published:𝔹, mutation:IRI⟩
  Store ≜ ⟨events:Event*⟩
}

⟦Γ:Invariants⟧{
  ∀s:Store, e:Event ∈ s.events:
    crash_and_restart(s) ⇒ ◊ (e.published = true)
  ∀s:Store, e₁ e₂:Event ∈ s.events:
    e₁.seq < e₂.seq ⇒ delivered(e₁) ≺ delivered(e₂)
  no_event_lost ≜ ∀e ∈ pre_crash_events: e ∈ post_recovery_events
}

⟦Λ:Scenario⟧{
  given ≜ dispatch_in_flight ∧ ∃e:Event ∈ store.events: e.published = false
  when  ≜ kill(-9, dec) ; restart(dec)
  then  ≜ ◊≤T (e.published = true ∧ consumer_received(e))
  where T ≜ 5s
}

⟦Ε⟧⟨δ≜0.85;φ≜70;τ≜◊?⟩
