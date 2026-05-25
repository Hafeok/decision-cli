//! In-memory shape of a worker OCI image manifest (FT-088 / ADR-056).
//!
//! Plain data: the three things the admission flow needs to make a
//! manifest-only decision are the image config's labels, the manifest's
//! annotations, and the platforms declared on a multi-arch manifest
//! list. We do not attempt to model the full OCI manifest schema — the
//! caller is responsible for distilling whatever `docker manifest
//! inspect` (or a registry-native API) returned into this shape.

use std::collections::{BTreeMap, BTreeSet};

use super::labels::parse_capability_tag;

/// A platform descriptor as it appears on an OCI manifest list entry.
/// String form `"<os>/<arch>"` is the canonical comparison key
/// (e.g. `"linux/amd64"`) — exactly what `MIN_REQUIRED_PLATFORMS`
/// enumerates.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Platform {
    /// e.g. `"linux"`.
    pub os: String,
    /// e.g. `"amd64"`, `"arm64"`.
    pub arch: String,
}

impl Platform {
    /// Construct a platform from `os` and `arch` strings.
    #[must_use]
    pub fn new(os: impl Into<String>, arch: impl Into<String>) -> Self {
        Self {
            os: os.into(),
            arch: arch.into(),
        }
    }

    /// Canonical `"<os>/<arch>"` rendering.
    #[must_use]
    pub fn as_key(&self) -> String {
        format!("{}/{}", self.os, self.arch)
    }
}

/// Distilled view of a worker OCI image's manifest.
///
/// Caller responsibility: populate `labels` from the image config's
/// `Labels` field, `annotations` from the manifest's `annotations` map,
/// and `platforms` from the manifest list's platform descriptors.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct WorkerOciManifest {
    /// Image config labels (`config.Labels`). Keyed by label name; values
    /// are the literal label values.
    pub labels: BTreeMap<String, String>,
    /// Manifest-level annotations (`manifest.annotations`).
    pub annotations: BTreeMap<String, String>,
    /// Platforms the manifest list publishes. For a single-arch image
    /// the caller should populate this with the single supported
    /// platform; the validator decides whether it meets the multi-arch
    /// floor.
    pub platforms: Vec<Platform>,
}

impl WorkerOciManifest {
    /// Construct an empty manifest. Convenience for tests; production
    /// callers build directly via struct-literal syntax.
    #[must_use]
    pub fn empty() -> Self {
        Self::default()
    }

    /// Extract the set of capability tags this manifest claims, by
    /// walking [`labels`](Self::labels) for keys carrying the
    /// `ddd.capability-tag.` prefix with value `"true"`. Order is
    /// deterministic (`BTreeSet`) so consumers can fold over the
    /// result without surprise.
    #[must_use]
    pub fn capability_tags(&self) -> BTreeSet<String> {
        self.labels
            .iter()
            .filter_map(|(k, v)| parse_capability_tag(k.as_str(), v.as_str()).map(String::from))
            .collect()
    }

    /// Read the SDK version label, if present.
    #[must_use]
    pub fn sdk_version(&self) -> Option<&str> {
        self.labels
            .get(super::LABEL_DDD_SDK_VERSION)
            .map(String::as_str)
    }

    /// Read the wire-protocol label, if present.
    #[must_use]
    pub fn wire_protocol(&self) -> Option<&str> {
        self.labels
            .get(super::LABEL_DDD_WIRE_PROTOCOL)
            .map(String::as_str)
    }

    /// Read the OCI source-repository annotation, if present.
    #[must_use]
    pub fn source_repo(&self) -> Option<&str> {
        self.annotations
            .get(super::OCI_ANNOTATION_SOURCE)
            .map(String::as_str)
    }

    /// Read the OCI revision (commit hash) annotation, if present.
    #[must_use]
    pub fn revision(&self) -> Option<&str> {
        self.annotations
            .get(super::OCI_ANNOTATION_REVISION)
            .map(String::as_str)
    }

    /// Return the set of platform keys (`"<os>/<arch>"`) the manifest declares.
    #[must_use]
    pub fn platform_keys(&self) -> BTreeSet<String> {
        self.platforms.iter().map(Platform::as_key).collect()
    }
}
