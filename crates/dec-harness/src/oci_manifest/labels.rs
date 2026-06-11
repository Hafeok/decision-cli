//! Stable constants for worker OCI label / annotation keys (FT-088 / ADR-057).
//!
//! These constants are the only place the wire strings appear; every
//! consumer (validator, future bundle assembler for FT-094) imports from
//! here. Renaming a label is a schema migration and requires editing
//! exactly one file.

/// OCI label prefix used to claim capability tags on a worker image.
///
/// Per ADR-057, each tag a worker claims is recorded as a separate label
/// of the form `ddd.capability-tag.<tag>=true`, where `<tag>` is the
/// capability identifier. The "one label per tag" shape — rather than a
/// single comma-separated `ddd.capability-tags=a,b,c` label — keeps the
/// manifest queryable: a registry-side filter for "images claiming tag
/// X" is a substring match on the label key, no value parsing required.
pub const CAPABILITY_TAG_LABEL_PREFIX: &str = "ddd.capability-tag.";

/// OCI label key pinning the worker SDK version baked into the image.
/// Value MUST be a semver string (`major.minor.patch`).
pub const LABEL_DDD_SDK_VERSION: &str = "ddd.sdk-version";

/// OCI label key pinning the SSE/POST wire-protocol version the image's
/// long-running entrypoint speaks. Value MUST be a semver string.
pub const LABEL_DDD_WIRE_PROTOCOL: &str = "ddd.wire-protocol";

/// Standard OCI annotation pointing at the source repository the image
/// was built from. Value is a URL (e.g. `https://github.com/example/worker`).
pub const OCI_ANNOTATION_SOURCE: &str = "org.opencontainers.image.source";

/// Standard OCI annotation pinning the commit hash the image was built from.
/// Value is the full git SHA.
pub const OCI_ANNOTATION_REVISION: &str = "org.opencontainers.image.revision";

/// The platforms an admission-eligible worker image MUST declare in its
/// manifest list. ADR-056's "multi-arch where reasonable" floor; the
/// admission validator refuses any image missing either of these.
pub const MIN_REQUIRED_PLATFORMS: &[&str] = &["linux/amd64", "linux/arm64"];

/// Construct the OCI label key for a single capability tag.
///
/// `tag` is the capability identifier (e.g. `"code-writer"`); the result
/// is the label key (e.g. `"ddd.capability-tag.code-writer"`). Callers
/// pair this with the literal value `"true"` per ADR-057.
#[must_use]
pub fn capability_tag_label(tag: &str) -> String {
    format!("{CAPABILITY_TAG_LABEL_PREFIX}{tag}")
}

/// Parse the capability tag out of a label key, if it carries the
/// capability prefix and the value is `"true"`. Returns `None` for any
/// other label (the canonical predicate "does this label declare a
/// capability tag?").
#[must_use]
pub fn parse_capability_tag<'a>(label_key: &'a str, label_value: &str) -> Option<&'a str> {
    if label_value != "true" {
        return None;
    }
    let suffix = label_key.strip_prefix(CAPABILITY_TAG_LABEL_PREFIX)?;
    if suffix.is_empty() {
        return None;
    }
    Some(suffix)
}
