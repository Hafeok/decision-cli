---
id: ADR-062
title: No worker supervisor in slice 1; humans run workers manually
status: accepted
features:
- FT-095
supersedes: []
superseded-by: []
domains: []
scope: feature-specific
content-hash: sha256:5d28e06b3f1c4b1bcdfbe5cdff54d49b6a835f92a338cce19850379dfccd3be3
---

## Context

A WorkerSupervisor service — instructed by the orchestration system to spawn and scale worker instances based on dispatch demand and binding state — is the right long-term answer. Slice 1 doesn't have demand variation, doesn't have multi-tenancy, and doesn't have autonomous scale-up policy. Building the Supervisor in slice 1 means writing infrastructure for state that doesn't yet exist.

## Decision

Defer the WorkerSupervisor to slice 4+. Slice 1 ships only `pipeline-cli workers run <worker-image-id>` and a manual runtime stance: the operator reads orchestration binding state and runs one process per capability tag they want covered.

Risk this defers: orchestration cannot autonomously bring workers up or down based on dispatch demand. Capability tags with no running worker process result in dispatches that escalate to humans. Acceptable in slice 1 because the operator IS the human running workers and the escalation loop is fast.

Progression:
- **Slice 2-3:** `pipeline-cli workers compose` generates a `docker-compose.yml` from current eligibility + binding state. Restart policy lives in compose; still no autonomous decisions.
- **Slice 4+:** Real WorkerSupervisor service.

## Consequences

- **Positive:** Slice 1 scope shrinks substantially. No daemon, no autoscale logic, no demand metrics.
- **Positive:** The `workers run` subcommand is the same surface the eventual Supervisor wraps; nothing to throw away.
- **Negative:** Tags without running workers stall dispatches. Mitigated by the operator being the supervisor and the orchestration system surfacing "no worker for tag X" clearly.
- **Negative:** Multi-worker startup is a manual loop. Acceptable in single-tenant slice 1.

## Alternatives considered

- **Build the Supervisor in slice 1.** Rejected — speculative infrastructure ahead of the demand patterns it has to manage.
- **systemd / init-script per worker.** Rejected — couples to host init system, doesn't survive a move to a different host shape, and the manual stance is enough for slice 1.

## References

- `brief:worker-distribution-slice-1`
- `feature:worker-supervisor` (excluded; slice 4+).
