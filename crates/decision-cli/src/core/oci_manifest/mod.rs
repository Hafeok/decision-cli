//! Worker OCI image packaging conventions (FT-088 / ADR-056 / ADR-057).
//!
//! Every worker OCI image MUST carry a fixed set of OCI labels and
//! annotations so the orchestration catalog can index it, route by
//! capability, and audit its provenance *without pulling the image*.
//! The conventions are:
//!
//! - **Capability tags** — one label per claimed tag, named
//!   `ddd.capability-tag.<tag>=true`. Per ADR-057, this is the
//!   manifest-level index the catalog consults during shallow operations.
//! - **SDK version** — `ddd.sdk-version=<semver>`. Pins the worker SDK
//!   version baked into the image so the harness can detect wire-protocol
//!   drift before dispatching.
//! - **Wire protocol** — `ddd.wire-protocol=<semver>`. The SSE/POST contract
//!   between the worker and `pipeline-cli` (per `feature:manual-runtime-stance`).
//! - **Source provenance** — standard OCI annotations
//!   `org.opencontainers.image.source=<repo-url>` and
//!   `org.opencontainers.image.revision=<commit-sha>` link the image back
//!   to the exact source it was built from.
//! - **Multi-arch** — at minimum `linux/amd64` and `linux/arm64` platforms
//!   are declared in the image's manifest list.
//!
//! These conventions are enforced at the `pipeline-cli` admission boundary
//! by [`validate_worker_oci_manifest`]: a candidate
//! [`WorkerOciManifest`] missing any required label / annotation /
//! platform is rejected before the `WorkerCurator` (FT-092) ever sees it.
//!
//! This module is intentionally substrate-only: it operates on plain
//! `BTreeMap<String, String>` label / annotation collections (the form
//! `docker manifest inspect` produces) and a list of supported platforms.
//! It does not call out to a registry, pull bytes, or talk to the graph.
//! The caller is responsible for sourcing the inputs (FT-094 will wire
//! this into the `WorkerImageSubmission` admission flow).

mod labels;
mod manifest;
mod validate;

#[cfg(test)]
mod tests;

pub use labels::{
    capability_tag_label, parse_capability_tag, CAPABILITY_TAG_LABEL_PREFIX,
    LABEL_DDD_SDK_VERSION, LABEL_DDD_WIRE_PROTOCOL, MIN_REQUIRED_PLATFORMS,
    OCI_ANNOTATION_REVISION, OCI_ANNOTATION_SOURCE,
};
pub use manifest::{Platform, WorkerOciManifest};
pub use validate::{
    validate_worker_oci_manifest, OciManifestValidationError, OciManifestViolation,
};
