---
id: FT-088
title: 'worker-distribution: OCI image packaging conventions every worker must follow'
phase: 3
status: planned
depends-on: []
adrs:
- ADR-056
- ADR-057
tests: []
domains: []
domains-acknowledged: {}
---

## Motivation

Derived from `brief:worker-distribution-slice-1`. The catalog discipline only works if every WorkerImage exposes its capability claims, SDK version, and wire-protocol version in a uniform, machine-readable form on the manifest. Addresses ADR-056 (OCI format) and ADR-057 (capability tags as OCI labels).

## Scope

A worker OCI image MUST:

- Carry capability tags as OCI labels: `ddd.capability-tag.<tag>=true` per tag claimed. Machine-readable from the manifest without pulling the image.
- Pin the worker SDK version: `ddd.sdk-version=<semver>`.
- Pin the wire-protocol version: `ddd.wire-protocol=<semver>`.
- Declare a long-running worker entrypoint that opens the SSE connection to pipeline-cli on start, reading endpoint and bearer token from environment variables (per `feature:manual-runtime-stance`).
- Be multi-arch where reasonable (at least `linux/amd64` and `linux/arm64`).
- Carry an OCI annotation pointing to the source repo and commit hash.

Slice 1 ships a base image `pipeline-worker-base:<version>` that worker authors extend. The base bakes in the SDK and the SSE/POST loop; authors add their worker logic and metadata labels.

The `pipeline-cli` SHACL validation for `WorkerImageSubmission` checks that the candidate image's manifest carries the required labels before admission proceeds.

## Out of scope

- Base image build pipeline (provisioned operationally; reusable workflow consumes it).
- WASM runtime variant (`feature:wasm-runtime`, excluded).
- Dagger runtime variant (`feature:dagger-runtime`, excluded per ADR-065).

## Success criteria

- A worker image built per these conventions is queryable for capability tags via `docker manifest inspect` without pulling.
- A worker image missing a required label is rejected by the admission flow before reaching the Curator's bundle.
- The base image documents the convention and is referenced from worker repo templates.
