---
id: TC-441
title: cell payload tool surface equals role catalog surface when cell declares nothing
type: exit-criteria
status: unimplemented
validates:
  features: []
  adrs: []
phase: 1
runner: cargo-test
runner-args: -p decision-cli ft_174_cell_surface_from_catalog
runner-timeout: 300
observes:
- file
- graph
---

## Description

A fixture cluster run uses a stub worker that records the `DispatchPayload` it receives. The orchestration store seeds the implementer role with a known `dec:roleTool` set; the dispatched cell declares no `tools` narrowing. Asserts on **file** (the stub-recorded payload's `allowed_tools` equals the seeded role surface exactly — proving the `cluster_dispatch.rs` hardcode is gone) and on the **graph** (the role's `dec:roleTool` quads in the store are the values that arrived in the payload).
