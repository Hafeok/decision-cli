---
id: TC-468
title: cluster cells reach semantic tools over a worktree-of-HEAD plus sandbox overlay
type: scenario
status: unimplemented
validates:
  features: []
  adrs: []
phase: 1
runner: cargo-test
runner-args: -p decision-cli ft_180_sandbox_overlay_intel
runner-timeout: 600
observes:
- disk-state
- stdout
---

## Description

A cluster cell granted semantic read tools queries them against the FT-172-style construction: a worktree of HEAD with the sandbox's cell outputs grafted as an overlay. Asserts on **disk-state** (the temporary worktree is created for the run and removed unconditionally afterwards; the sandbox itself is never mutated by reads) and **stdout** (an outline query over a sandbox-emitted file resolves through the overlay — proving cells see HEAD + their own outputs as one workspace).
