---
id: TC-133
title: CycloneDX SBOM is reachable as the image's OCI referrer
type: exit-criteria
status: passing
validates:
  features: []
  adrs: []
phase: 1
runner: cargo-test
runner-args: tc_133_cyclonedx_sbom_is_reachable_as_the_image_s_oci_ref
runner-timeout: 120
last-run: 2026-05-26T00:38:17.914540397+00:00
last-run-duration: 26.2s
---

## Description

Exit criterion for [FT-091](FT-091): the CycloneDX SBOM produced during
the release workflow is reachable as an OCI referrer of the image, and
the orchestration substrate refuses any WorkerImageSubmission whose
declared SBOM reference is missing or malformed.

## Given

A WorkerImageSubmission carries a `claimed_sbom_ref` field. Per FT-091 /
ADR-059, that value MUST be a syntactically-correct OCI referrer
descriptor URI — concretely, a digest-pinned OCI reference of the shape
`<registry>/<repository>@sha256:<64 lowercase hex digits>`. The release
workflow attaches the CycloneDX SBOM to the image via `cosign attach
sbom`, after which `cosign download sbom <image-ref>` resolves the
attached SBOM via the registry's referrers API at this digest.

## When

`cargo test -p decision-cli --test
tc_133_cyclonedx_sbom_is_reachable_as_the_image_s_oci_ref
tc_133_cyclonedx_sbom_is_reachable_as_the_image_s_oci_ref` runs the
checkpoint test in
`crates/decision-cli/tests/tc_133_cyclonedx_sbom_is_reachable_as_the_image_s_oci_ref.rs`.

## Then

The checkpoint test asserts six claims end-to-end:

1. The canonical OCI referrer descriptor URI shape admits via
   `core::sbom_referrer::validate_oci_referrer_uri`, and the parsed
   components round-trip back to the same URI.
2. The SHACL validator on `dec:WorkerImageSubmission` admits a Submission
   whose `claimed_sbom_ref` carries that canonical URI.
3. The SHACL validator refuses Submissions whose `claimed_sbom_ref` is a
   mutable tag (`:latest`), a short digest, or uses a non-`sha256`
   algorithm. Every violation cites `dec:claimed_sbom_ref`.
4. `core::sbom_referrer::assemble_curator_submission_bundle` produces a
   bundle that exposes the SBOM referrer URI verbatim for the
   WorkerCurator (FT-092) to cite in the admission verdict.
5. The same assembler REFUSES a Submission whose `claimed_sbom_ref` is
   empty — the explicit FT-091 success criterion: *"bundle assembly
   fails when the SBOM is declared missing on a Submission."*
6. The same assembler refuses a Submission whose `claimed_sbom_ref` is
   non-empty but malformed (with a `SbomMalformed` error variant).

Exit code 0 = pass. Any failure surfaces with the case label.

## Notes

- The release workflow itself (`.github/workflows/release-worker.yml`)
  carries the `syft` SBOM generation + `cosign attach sbom` steps but
  cannot be exercised at unit-test latency. The checkpoint test instead
  pins the *contract* the workflow produces — a Submission carrying a
  canonical OCI referrer descriptor URI — and verifies that the
  admission substrate accepts it and refuses the negative cases.
- The four-character cosign-canonical sha256 prefix for tests is
  `cafebabe` (vs `deadbeef` for the registry ref) so violations report
  legible distinct hex bodies.