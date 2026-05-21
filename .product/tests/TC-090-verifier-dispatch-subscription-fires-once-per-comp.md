---
id: TC-090
title: verifier dispatch subscription fires once per completed implementer session
type: exit-criteria
status: unrunnable
validates:
  features:
  - FT-022
  adrs: []
phase: 1
runner: cargo-test
runner-args: tc_090_verifier_dispatch_subscription_fires_once
runner-timeout: 120
---

## Purpose

Exit criterion for [FT-022](FT-022): the verifier-dispatch subscription emits exactly one `VerifierDispatchEvent` for each completed implementer session, and never re-fires for the same session.

## Given

A clean store with the subscription registered. An implementer session `S1` is opened, runs to completion, and emits its completion event.

## When

The subscription processes the event stream up to and including `S1`'s completion event.

## Then

- Exactly one `VerifierDispatchEvent` is emitted with `triggered_by_session = S1`.
- Replaying the event stream (e.g. restart the subscription from an earlier sequence number) does **not** re-emit a duplicate event for `S1` — the subscription's ledger or event-stream dedup ensures idempotency.
- Two distinct implementer sessions produce two distinct dispatch events with distinct `bundle_hash` values.

## Notes

Pairs with the invariant TC-028 (`DispatchGroup` reaches `complete` only when both paired sessions resolve) by closing the upstream half: this TC asserts the dispatch event *fires correctly*; TC-028 asserts the resulting pair *completes correctly*.
