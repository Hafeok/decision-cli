---
id: FT-106
title: 'decision-cli: Cross-platform cargo-dist release flow and MCP registry publishing for the absorbed workspace'
phase: 4
status: planned
depends-on:
- FT-105
- FT-136
adrs:
- ADR-067
- ADR-061
- ADR-058
- ADR-077
tests:
- TC-180
- TC-181
- TC-182
domains: []
domains-acknowledged:
  observability: FT-106 carries TC-180, TC-181, TC-182 from the original ADR-067 plan; all three survive the ADR-077 re-scope (schema validation, lockstep, per-platform parity). The re-scoped slice is purely subtractive (drop product-shim references, drop dual-publish branch), so existing TCs continue to apply with narrowed scope. ADR-072 spans api + observability; the observability concerns are covered by the schema-validity gate (TC-181) which observes the on-disk server.json against the live registry schema. Explicit acknowledgement per ADR-072 review gate.
---

## Description

> **Re-scoped under [ADR-077](ADR-077).** This spec was originally authored under [ADR-067](ADR-067) to ship two binaries (`dec` + a `product` deprecation shim) and two `server.json` registry entries from this workspace. ADR-077 supersedes ADR-067's transport choice — `crates/product-shim/` is dropped, `crates/product-cli/` is dropped, and the standalone `product-cli` repo continues to publish `io.github.Hafeok/product-cli` from its own release flow. The original body is preserved in git history; this re-scoped body describes the work that actually remains.

Sibling slice to [FT-136](FT-136). Where FT-136 consumes `product-core` as a Cargo dep and deletes `crates/product-cli/` + `crates/product-shim/`, this slice re-aligns the cargo-dist release flow and the MCP registry publishing path to the single-binary shape ADR-077 leaves behind. What survives from the original FT-106:

- One `dec` binary as the only cargo-dist app in this workspace.
- One `server.json` (`crates/decision-cli/server.json`) describing `io.github.Hafeok/decision-cli`, refined to drop the "absorbed product-cli surface" framing inherited from the ADR-067 plan.
- One `publish-mcp.yml` publish entry, not two.
- The same five target triples, the same installer set, the same cargo-dist version pin.

The slice is mostly subtractive: drop `crates/product-shim` from `dist-workspace.toml`'s `members`; regenerate `release.yml` via `dist generate`; delete `crates/product-cli/server.json`; strip the dual-publish branch from `publish-mcp.yml`; re-word the surviving `server.json`'s description.

One subcommand → one slice — the slice is operational rather than verb-shaped. It modifies four files and adds nothing new.

## Functional Specification

### Inputs

- A landed [FT-136](FT-136), or both slices landing in the same PR. See "Sequencing" below.
- The current `dist-workspace.toml`, `crates/decision-cli/server.json`, `crates/product-cli/server.json`, and `.github/workflows/publish-mcp.yml` as authored under the original ADR-067 plan.
- The MCP registry schema (`https://static.modelcontextprotocol.io/schemas/2025-09-29/server.schema.json`).
- cargo-dist v0.31.0 (current pin) or later — bumping is allowed but not required.

### Outputs

- `dist-workspace.toml` with `members = ["cargo:crates/decision-cli"]` only — `crates/product-shim` removed; comment block updated to reflect the single-binary shape.
- Regenerated `.github/workflows/release.yml` via `dist generate`.
- `crates/decision-cli/server.json` with the "Includes the absorbed product-cli surface; replaces io.github.Hafeok/product-cli for new installations" framing replaced by a stand-alone description.
- `crates/product-cli/server.json` deleted (FT-136 deletes the entire `crates/product-cli/` directory; this slice asserts the deletion at the release-flow surface).
- `.github/workflows/publish-mcp.yml` rewritten so the publish matrix has one entry (`decision-cli` / `dec-x86_64-unknown-linux-gnu.tar.xz`) instead of two.
- A CI smoke-test that validates `crates/decision-cli/server.json` against the registry schema on every PR.

### State

- Removed on-disk: `crates/product-cli/server.json` (joint with FT-136's full directory deletion).
- Updated on-disk: `dist-workspace.toml`, `.github/workflows/release.yml`, `.github/workflows/publish-mcp.yml`, `crates/decision-cli/server.json`.
- Preserved on-disk: cargo-dist version pin, target triples (`aarch64-apple-darwin`, `aarch64-unknown-linux-gnu`, `x86_64-apple-darwin`, `x86_64-unknown-linux-gnu`, `x86_64-pc-windows-msvc`), installer set (`shell`, `powershell`, `homebrew`), hosting backend (`github`).

### Behaviour

#### Sequencing

This slice and [FT-136](FT-136) may land in **one PR** or in **two**. Cleanest review is one PR (the surface is small and the failure modes are coupled). If split: FT-136 lands first (deletes `crates/product-cli/` and `crates/product-shim/`), then this slice lands second (updates `dist-workspace.toml` + `publish-mcp.yml` + the surviving `server.json` to match). Between the two PRs the release flow is broken — that's the cost of splitting; mitigated by not tagging a release between the two.

#### Phase 1 — `dist-workspace.toml` cleanup

1. Edit `dist-workspace.toml`:
   - `members = ["cargo:crates/decision-cli"]` (drop `"cargo:crates/product-shim"`).
   - Update the comment block to describe the single-binary shape and the ADR-077 supersession.
2. Run `dist generate` locally to regenerate `release.yml`.
3. Commit the regenerated `release.yml` alongside `dist-workspace.toml` so subsequent `dist generate` runs are no-ops.

#### Phase 2 — `server.json` cleanup for `dec`

Edit `crates/decision-cli/server.json`:
- `description`: rewrite to stand alone — e.g. *"Orchestration system for Decision-Driven Design — drives an external product-cli through the engineering process via LLM-backed role dispatch."*. Drop the "absorbed product-cli surface" and "replaces io.github.Hafeok/product-cli" framing.
- `name`, `repository`, `packages`, `runtimeArguments`, the `fileSha256`/`version` placeholders are unchanged.

Delete `crates/product-cli/server.json` (covered by FT-136's directory deletion if landing together; explicit `git rm` if this slice lands separately first).

#### Phase 3 — `publish-mcp.yml` cleanup

1. Strip the `product-cli` branch from the publish matrix (or the second sequential publish step, depending on how the workflow was authored).
2. Keep the `decision-cli` / `dec-x86_64-unknown-linux-gnu.tar.xz` publish path.
3. `MCPB_TARGET` and `MCP_PUBLISHER_VERSION` env vars unchanged.
4. `continue-on-error: true` on the single publish step is retained (registry-side failures still don't block the GitHub Release).

#### Phase 4 — CI schema validation

The schema-validity gate from the original FT-106 §Phase 6 survives, narrowed to one server.json instead of two. A workspace test or CI step parses `crates/decision-cli/server.json` against the registry's `server.schema.json` on every PR.

### Invariants

- **One cargo-dist app.** `dist-workspace.toml`'s `members` array names `crates/decision-cli` only.
- **One server.json published to the MCP registry from this workspace.** `io.github.Hafeok/decision-cli`. The `io.github.Hafeok/product-cli` entry continues to be published from the standalone product-cli repo's own release flow — this workspace is not the publisher.
- **Five target triples preserved.** A cargo-dist target-list regression fails CI (carried from the original FT-106 invariant).
- **`server.json` schema-valid on every commit.** Asserted by the Phase 4 CI step.
- **Registry-publish failure is non-fatal.** Inherited.
- **`release.yml` is autogenerated, not hand-edited.** Inherited; the source of truth is `dist-workspace.toml`.

### Error handling

- **`dist generate` produces a different `release.yml` than what's committed** → CI workflow-parity check fails; regenerate and commit.
- **`server.json` schema violation** → PR CI fails with the validator's diagnostic.
- **MCP-registry rejection at publish time** → logged in the workflow's step summary; retry via `workflow_dispatch` after fixing.
- **Cargo-dist version drift across crates** → the lockstep-version check is dropped (only one crate ships a binary); a version-bump regression on a non-binary crate is no longer a release-flow concern.

### Boundaries

- **In scope.** The four phases above; cleanup of artifacts authored under the original ADR-067 plan; the narrowed schema-validation CI gate.
- **Out of scope.** Removing the `io.github.Hafeok/product-cli` registry entry (forever-alive; the standalone repo's responsibility). Multi-platform MCPB packages. Cargo-dist version bumps (separate maintenance task). Re-running prior verification graphs (separate sweep). All of FT-136's responsibilities (source-code rewrites, dependency wiring, MCP server merge). Removing the standalone `product` binary or its release flow.

## Out of scope

- Removing the legacy registry entry.
- Multi-platform MCPB packages.
- Cargo-dist version bumps.
- Switching release tooling.
- Re-running prior verification sweeps.
- Documentation overhaul beyond minor comment-block updates.
- Source-code, dependency, or MCP-server changes (those live in FT-136).
