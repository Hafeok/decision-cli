---
id: TC-448
title: accepted split records derivation provenance and sibling depends-on edges
type: scenario
status: unimplemented
validates:
  features: []
  adrs: []
phase: 1
runner: cargo-test
runner-args: -p decision-cli ft_175_split_provenance_edges
runner-timeout: 300
observes:
- graph
- file
---

## Description

Applying an accepted split proposal must leave the product graph in the post-split shape: child feature files exist on disk (**file** surface — front-matter parses, bodies carry the required sections), each child links `depends-on` to its prescribed siblings, the parent links its children and is marked as the split umbrella, and the derivation provenance (split-from edge plus the triggering size signals) is queryable (**graph** surface). `product graph check` over the fixture stays structurally clean after the apply.
