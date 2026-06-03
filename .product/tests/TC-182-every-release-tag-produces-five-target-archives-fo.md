---
id: TC-182
title: Every release tag produces five target archives for the dec binary
type: exit-criteria
status: passing
validates:
  features:
  - FT-106
  adrs: []
phase: 1
runner: bash
runner-args: tests/scripts/tc-182-release-artifact-parity.sh
runner-timeout: 60
last-run: 2026-06-03T12:20:26.933108738+00:00
last-run-duration: 0.0s
---

## Claim

Every release tag pushed to the decision-cli workspace produces five target-platform archives for the `dec` binary — `aarch64-apple-darwin`, `aarch64-unknown-linux-gnu`, `x86_64-apple-darwin`, `x86_64-unknown-linux-gnu`, `x86_64-pc-windows-msvc`. Re-scoped under ADR-077: only the dec binary is shipped from this workspace (product-cli was dropped).

## Scenarios

### Setup

- A tagged release exists in the workspace's GitHub Releases (the test asserts against a real release, either the most recent or a specifically named one).
- `gh` CLI (or equivalent GitHub API access) available to enumerate release assets.
- The test ships an expected-archives manifest:

```
EXPECTED_TARGETS=(
  aarch64-apple-darwin
  aarch64-unknown-linux-gnu
  x86_64-apple-darwin
  x86_64-unknown-linux-gnu
  x86_64-pc-windows-msvc
)
EXPECTED_BINARY=dec
EXPECTED_ARCHIVE_EXTS=(
  tar.xz tar.xz tar.xz tar.xz zip  # parallel to EXPECTED_TARGETS; Windows uses zip
)
```

### Scenario A — every target has an archive

For the most recent release tag, list all release assets. Assertions:
- For each `target ∈ EXPECTED_TARGETS`, an asset named `dec-<target>.<ext>` exists.
- Total archive count: 5 archives (one per target), plus the installer scripts (`installer.sh`, `installer.ps1`, Homebrew formula) that cargo-dist auto-generates.
- A missing archive fails the test with a diagnostic naming the absent target.

### Scenario B — version consistency

For each archive, extract the binary and run `dec --version`. Assertions:
- Every archive's binary reports the same SemVer string.
- The reported version matches the release tag (modulo the `v` prefix).
- The reported version matches the version in `crates/decision-cli/Cargo.toml`.

### Scenario C — archive contents are minimal

Each archive contains exactly:
- The `dec` binary.
- A license file (LICENSE).
- A README or doc subset (per cargo-dist's defaults).
- No stray `.git/`, `target/`, or other build artefacts.

Asserted via `tar -tzf` (or `unzip -l` for Windows) and a contents-allowlist.

### Scenario D — archive checksums are present

For each archive, the release also publishes a `.sha256` or equivalent checksum file (cargo-dist does this by default via the `dist-manifest.json` it uploads). Assertions:
- Either per-archive `.sha256` files exist, OR the release has a `dist-manifest.json` listing every archive's SHA-256.
- For Scenario A's full set of 5 archives, every one has a recorded SHA matching what `sha256sum` produces on download.

### Scenario E — installer scripts work end-to-end

Run the shell installer (`installer.sh`) in an isolated environment (a container or a temp `HOME`). Assertions:
- Exit 0.
- The `dec` binary is installed to the expected location.
- The binary is executable and responds to `--version`.

Repeat for the PowerShell installer on a Windows runner if available. Homebrew formula testing is brittle in CI; skip with a documented note.

### Scenario F — dist-workspace.toml structural check (pre-release)

A separate sub-assertion that runs on PRs, not against releases: validate that `dist-workspace.toml` has `members = ["cargo:crates/decision-cli"]` only and includes all five target triples. Drift fails CI before any tag push.

## Runner

`bash tests/scripts/tc-182-release-artifact-parity.sh`. Two modes:

1. **Release-mode** — invoked with `--tag v0.X.Y`; queries the GitHub Releases API for that tag and runs Scenarios A-E. This mode runs on a schedule (nightly checks the most recent release) and on-demand for release validation.
2. **PR-mode** — invoked with no args; runs Scenario F only (dist-workspace.toml structural check). This mode runs on every PR.

The two modes share the same script but gate scenarios on `--tag` presence.

## Non-goals

- Validating that the installed binaries work correctly on each platform (TC-178 covers `cargo test --workspace` portability for Linux/macOS x86_64; per-platform integration testing is a separate concern, typically handled by a matrix CI workflow).
- Validating Homebrew formula content (out of slice; cargo-dist auto-generates and the test would be brittle).
- MCPB-package validation (TC-180 covers the MCPB install path through the MCP registry).
- Per-platform performance comparison or feature-parity beyond `--version` (out of slice).
- Auto-creating a release tag (operators tag manually; this TC just verifies what's produced).